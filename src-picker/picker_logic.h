/**
 * Picker logic for XDPH parsing and output formatting.
 */

#ifndef PICKER_LOGIC_H
#define PICKER_LOGIC_H

#include <QString>
#include <QVector>
#include <cstdint>
#include <optional>

/**
 * Window entry from XDPH's window list.
 */
struct WindowEntry {
    uint64_t handleId;
    QString windowClass;
    QString title;
    uint64_t windowAddr;
};

/**
 * Parse the XDPH_WINDOW_SHARING_LIST environment variable.
 */
QVector<WindowEntry> parseWindowList(const QString& envValue);

/**
 * Find a window handle by Hyprland address.
 */
std::optional<uint64_t> findWindowHandle(const QVector<WindowEntry>& windows, uint64_t hyprlandAddr);

/**
 * Format monitor selection output for XDPH.
 */
QString formatMonitorOutput(const QString& sourceId);

/**
 * Format window selection output for XDPH.
 */
QString formatWindowOutput(const QString& sourceId);

/**
 * Format region selection output for XDPH.
 */
QString formatRegionOutput(const QString& sourceId, int x, int y, unsigned int width, unsigned int height);

/**
 * Run the fallback picker (hyprland-share-picker).
 * Returns exit code (0 for success, 1 for failure).
 */
int runFallbackPicker();

/**
 * Log a message to stderr and to /tmp/omnirec-picker.log.
 */
void pickerLog(const QString& msg);

#endif // PICKER_LOGIC_H
