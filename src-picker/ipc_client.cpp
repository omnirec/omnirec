/**
 * IPC client implementation using Qt's QLocalSocket.
 *
 * This connects to the omnirec-service's unified IPC interface using
 * length-prefixed JSON messages.
 */

#include "ipc_client.h"

#include <QLocalSocket>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QDir>
#include <QDataStream>
#include <QTextStream>
#include <cstdlib>
#include <iostream>
#include <unistd.h>

QString getSocketPath()
{
    // Use the unified service socket path
    QString runtimeDir = qEnvironmentVariable("XDG_RUNTIME_DIR");
    if (runtimeDir.isEmpty()) {
        runtimeDir = QString("/run/user/%1").arg(getuid());
    }
    return QDir(runtimeDir).filePath("omnirec/service.sock");
}

/**
 * Send a length-prefixed JSON message.
 */
static bool sendLengthPrefixedMessage(QLocalSocket& socket, const QJsonObject& message, QString* errorOut)
{
    QJsonDocument doc(message);
    QByteArray data = doc.toJson(QJsonDocument::Compact);
    
    // Write length prefix (4 bytes, little-endian)
    quint32 len = static_cast<quint32>(data.size());
    char lenBytes[4];
    lenBytes[0] = static_cast<char>(len & 0xFF);
    lenBytes[1] = static_cast<char>((len >> 8) & 0xFF);
    lenBytes[2] = static_cast<char>((len >> 16) & 0xFF);
    lenBytes[3] = static_cast<char>((len >> 24) & 0xFF);
    
    if (socket.write(lenBytes, 4) != 4) {
        if (errorOut) *errorOut = QString("Failed to write length prefix: %1").arg(socket.errorString());
        return false;
    }
    
    // Write message body
    if (socket.write(data) != data.size()) {
        if (errorOut) *errorOut = QString("Failed to write message body: %1").arg(socket.errorString());
        return false;
    }
    
    if (!socket.flush()) {
        if (errorOut) *errorOut = QString("Failed to flush socket: %1").arg(socket.errorString());
        return false;
    }
    
    return true;
}

/**
 * Read a length-prefixed JSON message.
 */
static QByteArray readLengthPrefixedMessage(QLocalSocket& socket, QString* errorOut)
{
    // Read length prefix (4 bytes)
    char lenBytes[4];
    qint64 bytesRead = 0;
    while (bytesRead < 4) {
        // Only wait if no data is available - data might already be buffered
        if (socket.bytesAvailable() == 0) {
            if (!socket.waitForReadyRead(5000)) {
                if (errorOut) *errorOut = QString("Timeout waiting for response length: %1").arg(socket.errorString());
                return QByteArray();
            }
        }
        qint64 n = socket.read(lenBytes + bytesRead, 4 - bytesRead);
        if (n <= 0) {
            if (errorOut) *errorOut = QString("Failed to read length prefix: %1").arg(socket.errorString());
            return QByteArray();
        }
        bytesRead += n;
    }
    
    // Parse length (little-endian)
    quint32 len = static_cast<quint32>(static_cast<unsigned char>(lenBytes[0])) |
                  (static_cast<quint32>(static_cast<unsigned char>(lenBytes[1])) << 8) |
                  (static_cast<quint32>(static_cast<unsigned char>(lenBytes[2])) << 16) |
                  (static_cast<quint32>(static_cast<unsigned char>(lenBytes[3])) << 24);
    
    // Validate length (max 64KB as per protocol)
    if (len > 65536) {
        if (errorOut) *errorOut = QString("Response too large: %1 bytes").arg(len);
        return QByteArray();
    }
    
    // Read message body
    QByteArray data;
    data.resize(static_cast<int>(len));
    bytesRead = 0;
    while (bytesRead < static_cast<qint64>(len)) {
        // Only wait if no data is available - data might already be buffered
        if (socket.bytesAvailable() == 0) {
            if (!socket.waitForReadyRead(5000)) {
                if (errorOut) *errorOut = QString("Timeout waiting for response body: %1").arg(socket.errorString());
                return QByteArray();
            }
        }
        qint64 n = socket.read(data.data() + bytesRead, len - bytesRead);
        if (n <= 0) {
            if (errorOut) *errorOut = QString("Failed to read response body: %1").arg(socket.errorString());
            return QByteArray();
        }
        bytesRead += n;
    }
    
    return data;
}

static IpcResponse parseResponse(const QByteArray& data, QString* errorOut)
{
    IpcResponse response;
    
    QJsonParseError parseError;
    QJsonDocument doc = QJsonDocument::fromJson(data, &parseError);
    
    if (parseError.error != QJsonParseError::NoError) {
        response.type = ResponseType::Error;
        response.errorMessage = QString("Failed to parse response: %1").arg(parseError.errorString());
        if (errorOut) *errorOut = response.errorMessage;
        return response;
    }
    
    QJsonObject obj = doc.object();
    QString type = obj["type"].toString();
    
    // Handle unified service responses
    if (type == "selection") {
        response.type = ResponseType::Selection;
        response.sourceType = obj["source_type"].toString();
        response.sourceId = obj["source_id"].toString();
        response.hasApprovalToken = obj["has_approval_token"].toBool(false);
        
        if (obj.contains("geometry")) {
            QJsonObject geomObj = obj["geometry"].toObject();
            Geometry geom;
            geom.x = geomObj["x"].toInt();
            geom.y = geomObj["y"].toInt();
            geom.width = static_cast<unsigned int>(geomObj["width"].toInt());
            geom.height = static_cast<unsigned int>(geomObj["height"].toInt());
            response.geometry = geom;
        }
    } else if (type == "no_selection") {
        response.type = ResponseType::NoSelection;
    } else if (type == "error") {
        response.type = ResponseType::Error;
        response.errorMessage = obj["message"].toString();
        if (errorOut) *errorOut = response.errorMessage;
    } else if (type == "token_valid") {
        response.type = ResponseType::TokenValid;
    } else if (type == "token_invalid") {
        response.type = ResponseType::TokenInvalid;
    } else if (type == "token_stored" || type == "ok") {
        response.type = ResponseType::TokenStored;
    } else {
        response.type = ResponseType::Error;
        response.errorMessage = QString("Unknown response type: %1").arg(type);
        if (errorOut) *errorOut = response.errorMessage;
    }
    
    return response;
}

IpcResponse querySelection(QString* errorOut)
{
    IpcResponse response;
    QString socketPath = getSocketPath();
    
    QLocalSocket socket;
    socket.connectToServer(socketPath);
    
    if (!socket.waitForConnected(3000)) {
        response.type = ResponseType::Error;
        response.errorMessage = QString("Failed to connect to service (is it running?): %1 (path: %2)")
            .arg(socket.errorString())
            .arg(socketPath);
        if (errorOut) *errorOut = response.errorMessage;
        return response;
    }
    
    // Send query_selection request using unified protocol
    QJsonObject request;
    request["type"] = "query_selection";
    
    if (!sendLengthPrefixedMessage(socket, request, errorOut)) {
        response.type = ResponseType::Error;
        response.errorMessage = errorOut ? *errorOut : "Failed to send request";
        return response;
    }
    
    QByteArray data = readLengthPrefixedMessage(socket, errorOut);
    if (data.isEmpty()) {
        response.type = ResponseType::Error;
        response.errorMessage = errorOut ? *errorOut : "Empty response";
        return response;
    }
    
    socket.disconnectFromServer();
    return parseResponse(data, errorOut);
}

bool storeToken(const QString& token, QString* errorOut)
{
    QString socketPath = getSocketPath();
    
    QLocalSocket socket;
    socket.connectToServer(socketPath);
    
    if (!socket.waitForConnected(3000)) {
        if (errorOut) *errorOut = QString("Failed to connect to service: %1 (path: %2)")
            .arg(socket.errorString())
            .arg(socketPath);
        return false;
    }
    
    // Send store_token request using unified protocol
    QJsonObject request;
    request["type"] = "store_token";
    request["token"] = token;
    
    if (!sendLengthPrefixedMessage(socket, request, errorOut)) {
        return false;
    }
    
    QByteArray data = readLengthPrefixedMessage(socket, errorOut);
    if (data.isEmpty()) {
        return false;
    }
    
    IpcResponse response = parseResponse(data, errorOut);
    socket.disconnectFromServer();
    
    if (response.type == ResponseType::TokenStored) {
        return true;
    } else if (response.type == ResponseType::Error) {
        if (errorOut) *errorOut = response.errorMessage;
        return false;
    } else {
        if (errorOut) *errorOut = "Unexpected response type";
        return false;
    }
}
