/**
 * Picker logic implementation.
 */

#include "picker_logic.h"

#include <QProcess>
#include <QFile>
#include <QTextStream>
#include <QDateTime>
#include <iostream>
#include <cstdlib>

void pickerLog(const QString& msg)
{
    // Print to stderr for manual testing
    std::cerr << msg.toStdString() << std::endl;
    
    // Write to log file
    QFile file("/tmp/omnirec-picker.log");
    if (file.open(QIODevice::Append | QIODevice::Text)) {
        QTextStream out(&file);
        qint64 timestamp = QDateTime::currentSecsSinceEpoch();
        out << "[" << timestamp << "] " << msg << "\n";
        file.close();
    }
}

QVector<WindowEntry> parseWindowList(const QString& envValue)
{
    QVector<WindowEntry> windows;
    QString remaining = envValue;
    
    while (!remaining.isEmpty()) {
        int idEnd = remaining.indexOf("[HC>]");
        if (idEnd == -1) break;
        QString idStr = remaining.left(idEnd);
        
        remaining = remaining.mid(idEnd + 5);
        int classEnd = remaining.indexOf("[HT>]");
        if (classEnd == -1) break;
        QString windowClass = remaining.left(classEnd);
        
        remaining = remaining.mid(classEnd + 5);
        int titleEnd = remaining.indexOf("[HE>]");
        if (titleEnd == -1) break;
        QString title = remaining.left(titleEnd);
        
        remaining = remaining.mid(titleEnd + 5);
        int addrEnd = remaining.indexOf("[HA>]");
        if (addrEnd == -1) break;
        QString addrStr = remaining.left(addrEnd);
        
        remaining = remaining.mid(addrEnd + 5);
        
        WindowEntry entry;
        entry.handleId = idStr.toULongLong();
        entry.windowClass = windowClass;
        entry.title = title;
        entry.windowAddr = addrStr.toULongLong();
        
        windows.append(entry);
    }
    
    return windows;
}

std::optional<uint64_t> findWindowHandle(const QVector<WindowEntry>& windows, uint64_t hyprlandAddr)
{
    for (const auto& w : windows) {
        if (w.windowAddr == hyprlandAddr) {
            return w.handleId;
        }
    }
    return std::nullopt;
}

QString formatMonitorOutput(const QString& sourceId)
{
    return QString("[SELECTION]/screen:%1").arg(sourceId);
}

QString formatWindowOutput(const QString& sourceId)
{
    // Parse the hyprland address from source_id (may be hex with 0x prefix)
    uint64_t hyprlandAddr;
    if (sourceId.startsWith("0x")) {
        hyprlandAddr = sourceId.mid(2).toULongLong(nullptr, 16);
    } else {
        hyprlandAddr = sourceId.toULongLong();
    }
    
    // Get window list from environment
    QString windowList = qEnvironmentVariable("XDPH_WINDOW_SHARING_LIST");
    QVector<WindowEntry> windows = parseWindowList(windowList);
    
    // Try to find the handle
    auto handle = findWindowHandle(windows, hyprlandAddr);
    if (handle.has_value()) {
        return QString("[SELECTION]/window:%1").arg(handle.value());
    } else {
        return QString("[SELECTION]/window:%1").arg(hyprlandAddr);
    }
}

QString formatRegionOutput(const QString& sourceId, int x, int y, unsigned int width, unsigned int height)
{
    return QString("[SELECTION]/region:%1@%2,%3,%4,%5")
        .arg(sourceId)
        .arg(x)
        .arg(y)
        .arg(width)
        .arg(height);
}

int runFallbackPicker()
{
    QString pickerBinary = qEnvironmentVariable("OMNIREC_FALLBACK_PICKER");
    if (pickerBinary.isEmpty()) {
        pickerBinary = "hyprland-share-picker";
    }
    
    std::cerr << "[omnirec-picker] Falling back to standard picker: " 
              << pickerBinary.toStdString() << std::endl;
    
    QProcess process;
    process.setProcessChannelMode(QProcess::ForwardedErrorChannel);
    process.start(pickerBinary, QStringList());
    
    if (!process.waitForStarted(5000)) {
        std::cerr << "[omnirec-picker] Failed to execute fallback picker '" 
                  << pickerBinary.toStdString() << "': " 
                  << process.errorString().toStdString() << std::endl;
        return 1;
    }
    
    process.waitForFinished(-1);
    
    // Forward stdout from fallback picker
    QByteArray output = process.readAllStandardOutput();
    if (!output.isEmpty()) {
        QString line = QString::fromUtf8(output).trimmed();
        std::cerr << "[omnirec-picker] Fallback picker output: " 
                  << line.toStdString() << std::endl;
        std::cout << line.toStdString() << std::endl;
    }
    
    return process.exitCode() == 0 ? 0 : 1;
}
