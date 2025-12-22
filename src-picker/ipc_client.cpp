/**
 * IPC client implementation using Qt's QLocalSocket.
 */

#include "ipc_client.h"

#include <QLocalSocket>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <QDir>
#include <QTextStream>
#include <cstdlib>
#include <iostream>

QString getSocketPath()
{
    QString runtimeDir = qEnvironmentVariable("XDG_RUNTIME_DIR");
    if (runtimeDir.isEmpty()) {
        runtimeDir = "/tmp";
    }
    return QDir(runtimeDir).filePath("omnirec/picker.sock");
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
    } else if (type == "token_stored") {
        response.type = ResponseType::TokenStored;
    } else {
        response.type = ResponseType::Error;
        response.errorMessage = QString("Unknown response type: %1").arg(type);
        if (errorOut) *errorOut = response.errorMessage;
    }
    
    return response;
}

static bool sendRequest(QLocalSocket& socket, const QJsonObject& request, QString* errorOut)
{
    QJsonDocument doc(request);
    QByteArray data = doc.toJson(QJsonDocument::Compact);
    data.append('\n');
    
    if (socket.write(data) != data.size()) {
        if (errorOut) *errorOut = QString("Failed to write to socket: %1").arg(socket.errorString());
        return false;
    }
    
    if (!socket.flush()) {
        if (errorOut) *errorOut = QString("Failed to flush socket: %1").arg(socket.errorString());
        return false;
    }
    
    return true;
}

static QByteArray readResponse(QLocalSocket& socket, QString* errorOut)
{
    // Wait for response (with timeout)
    if (!socket.waitForReadyRead(5000)) {
        if (errorOut) *errorOut = QString("Timeout waiting for response: %1").arg(socket.errorString());
        return QByteArray();
    }
    
    QByteArray response;
    while (socket.canReadLine() || socket.waitForReadyRead(100)) {
        QByteArray line = socket.readLine();
        if (!line.isEmpty()) {
            response = line.trimmed();
            break;
        }
    }
    
    if (response.isEmpty()) {
        if (errorOut) *errorOut = "Empty response from server";
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
        response.errorMessage = QString("Failed to connect to main app (is it running?): %1 (path: %2)")
            .arg(socket.errorString())
            .arg(socketPath);
        if (errorOut) *errorOut = response.errorMessage;
        return response;
    }
    
    // Send query_selection request
    QJsonObject request;
    request["type"] = "query_selection";
    
    if (!sendRequest(socket, request, errorOut)) {
        response.type = ResponseType::Error;
        response.errorMessage = errorOut ? *errorOut : "Failed to send request";
        return response;
    }
    
    QByteArray data = readResponse(socket, errorOut);
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
        if (errorOut) *errorOut = QString("Failed to connect to main app: %1 (path: %2)")
            .arg(socket.errorString())
            .arg(socketPath);
        return false;
    }
    
    // Send store_token request
    QJsonObject request;
    request["type"] = "store_token";
    request["token"] = token;
    
    if (!sendRequest(socket, request, errorOut)) {
        return false;
    }
    
    QByteArray data = readResponse(socket, errorOut);
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
