/**
 * IPC client for communicating with the main OmniRec app.
 *
 * Connects to the Unix socket server in the main app to query the current
 * capture selection when XDPH invokes us.
 */

#ifndef IPC_CLIENT_H
#define IPC_CLIENT_H

#include <QString>
#include <optional>

/**
 * Geometry for region capture.
 */
struct Geometry {
    int x;
    int y;
    unsigned int width;
    unsigned int height;
};

/**
 * Response type from the main app.
 */
enum class ResponseType {
    Selection,
    NoSelection,
    Error,
    TokenValid,
    TokenInvalid,
    TokenStored
};

/**
 * IPC response from main app to picker.
 */
struct IpcResponse {
    ResponseType type;

    // For Selection response
    QString sourceType;
    QString sourceId;
    std::optional<Geometry> geometry;
    bool hasApprovalToken = false;

    // For Error response
    QString errorMessage;
};

/**
 * Get the IPC socket path.
 */
QString getSocketPath();

/**
 * Query the main app for the current capture selection.
 * Returns the response, or an error response if connection failed.
 */
IpcResponse querySelection(QString* errorOut = nullptr);

/**
 * Store an approval token in the main app.
 * Returns true on success, false on failure with error message in errorOut.
 */
bool storeToken(const QString& token, QString* errorOut = nullptr);

#endif // IPC_CLIENT_H
