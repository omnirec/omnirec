/**
 * OmniRec Picker - Custom picker for xdg-desktop-portal-hyprland.
 *
 * This picker is invoked by xdg-desktop-portal-hyprland (XDPH) when a screencast
 * request needs source selection. It queries the main OmniRec app for the user's
 * capture selection and outputs it to stdout in XDPH format.
 *
 * Usage:
 *   Normal mode (invoked by XDPH):
 *     omnirec-picker
 *
 *   Dry-run mode (for testing the dialog):
 *     omnirec-picker --dry-run [--source-type monitor|window|region] [--source-id ID]
 */

#include "ipc_client.h"
#include "picker_logic.h"
#include "dialog.h"

#include <QApplication>
#include <QCommandLineParser>
#include <iostream>
#include <cstdlib>
#include <unistd.h>

/**
 * Print help message and exit.
 */
static void printHelp()
{
    std::cerr << "Usage: omnirec-picker [OPTIONS]\n"
              << "\n"
              << "Options:\n"
              << "  --dry-run              Test the dialog without IPC\n"
              << "  --source-type TYPE     Source type: monitor, window, region (default: monitor)\n"
              << "  --source-id ID         Source identifier (default: DP-1)\n"
              << "  --help, -h             Show this help\n";
}

/**
 * Parse command line arguments.
 */
struct Args {
    bool dryRun = false;
    QString sourceType = "monitor";
    QString sourceId = "DP-1";
    bool showHelp = false;
};

static Args parseArgs(int argc, char* argv[])
{
    Args args;
    
    for (int i = 1; i < argc; ++i) {
        QString arg = QString::fromUtf8(argv[i]);
        
        if (arg == "--dry-run") {
            args.dryRun = true;
        } else if (arg == "--source-type" && i + 1 < argc) {
            args.sourceType = QString::fromUtf8(argv[++i]);
        } else if (arg == "--source-id" && i + 1 < argc) {
            args.sourceId = QString::fromUtf8(argv[++i]);
        } else if (arg == "--help" || arg == "-h") {
            args.showHelp = true;
        }
    }
    
    return args;
}

/**
 * Run in dry-run mode - just test the dialog.
 */
static int runDryRun(const QString& sourceType, const QString& sourceId)
{
    std::cerr << "[dry-run] Testing dialog with source_type=" << sourceType.toStdString() 
              << ", source_id=" << sourceId.toStdString() << std::endl;
    
    DialogResult result = showApprovalDialog(sourceType, sourceId);
    
    switch (result) {
        case DialogResult::AlwaysAllow:
            std::cerr << "[dry-run] Result: APPROVED (always_allow=true)" << std::endl;
            {
                QString token = generateApprovalToken();
                std::cerr << "[dry-run] Generated token: " << token.toStdString() << std::endl;
                std::cerr << "[dry-run] (Token not stored in dry-run mode)" << std::endl;
            }
            return 0;
        case DialogResult::AllowOnce:
            std::cerr << "[dry-run] Result: APPROVED (always_allow=false)" << std::endl;
            return 0;
        case DialogResult::Denied:
        default:
            std::cerr << "[dry-run] Result: DENIED" << std::endl;
            return 1;
    }
}

/**
 * Main picker logic.
 */
static int runPicker()
{
    pickerLog("[omnirec-picker] === Picker started ===");
    pickerLog(QString("[omnirec-picker] PID: %1").arg(getpid()));
    
    // Log key environment variables
    QString runtimeDir = qEnvironmentVariable("XDG_RUNTIME_DIR");
    if (!runtimeDir.isEmpty()) {
        pickerLog(QString("[omnirec-picker] XDG_RUNTIME_DIR: %1").arg(runtimeDir));
    }
    
    pickerLog("[omnirec-picker] About to query selection...");
    
    QString error;
    IpcResponse response = querySelection(&error);
    
    if (response.type == ResponseType::Error && !error.isEmpty()) {
        pickerLog(QString("[omnirec-picker] Failed to query main app: %1").arg(error));
        return runFallbackPicker();
    }
    
    pickerLog("[omnirec-picker] Got response from IPC");
    
    switch (response.type) {
        case ResponseType::Selection: {
            pickerLog(QString("[omnirec-picker] Got selection: type=%1, id=%2, has_token=%3")
                .arg(response.sourceType)
                .arg(response.sourceId)
                .arg(response.hasApprovalToken ? "true" : "false"));
            
            // Track whether user selected "Always Allow"
            bool shouldStoreToken = false;
            QString token;
            
            // Check if we need to show approval dialog
            if (!response.hasApprovalToken) {
                pickerLog("[omnirec-picker] No approval token, showing dialog...");
                
                DialogResult dialogResult = showApprovalDialog(response.sourceType, response.sourceId);
                
                switch (dialogResult) {
                    case DialogResult::AlwaysAllow:
                        pickerLog("[omnirec-picker] User approved (always_allow=true)");
                        // Generate token now, but store it AFTER outputting to XDPH
                        token = generateApprovalToken();
                        shouldStoreToken = true;
                        break;
                    case DialogResult::AllowOnce:
                        pickerLog("[omnirec-picker] User approved (always_allow=false)");
                        break;
                    case DialogResult::Denied:
                        pickerLog("[omnirec-picker] User denied, exiting");
                        return 1;
                }
            } else {
                pickerLog("[omnirec-picker] Has approval token, auto-approving");
            }
            
            // Format output based on source type
            QString output;
            if (response.sourceType == "monitor") {
                output = formatMonitorOutput(response.sourceId);
            } else if (response.sourceType == "window") {
                output = formatWindowOutput(response.sourceId);
            } else if (response.sourceType == "region") {
                if (response.geometry.has_value()) {
                    const Geometry& geom = response.geometry.value();
                    output = formatRegionOutput(response.sourceId, geom.x, geom.y, geom.width, geom.height);
                } else {
                    std::cerr << "[omnirec-picker] Region selection missing geometry" << std::endl;
                    return 1;
                }
            } else {
                std::cerr << "[omnirec-picker] Unknown source type: " 
                          << response.sourceType.toStdString() << std::endl;
                return 1;
            }
            
            // Output to XDPH FIRST - this unblocks the portal immediately
            // This is critical: XDPH may kill the picker if stdout is not written quickly
            pickerLog(QString("[omnirec-picker] Output: %1").arg(output));
            std::cout << output.toStdString() << std::endl;
            std::cout.flush();  // Ensure output is flushed immediately
            
            // Now store the token (after XDPH has received our output)
            // If this fails or we're killed before it completes, recording still works
            if (shouldStoreToken) {
                pickerLog("[omnirec-picker] Storing approval token via IPC...");
                QString storeError;
                if (!storeToken(token, &storeError)) {
                    pickerLog(QString("[omnirec-picker] Failed to store token: %1").arg(storeError));
                    // Not fatal - recording will still work, just won't be persistent
                } else {
                    pickerLog("[omnirec-picker] Token stored successfully");
                }
            }
            
            pickerLog("[omnirec-picker] Exiting with SUCCESS");
            return 0;
        }
        
        case ResponseType::NoSelection:
            pickerLog("[omnirec-picker] No selection, using fallback picker");
            return runFallbackPicker();
        
        case ResponseType::Error:
            pickerLog(QString("[omnirec-picker] Error: %1").arg(response.errorMessage));
            return 1;
        
        default:
            pickerLog("[omnirec-picker] Unexpected response");
            return 1;
    }
}

int main(int argc, char* argv[])
{
    // Parse args before creating QApplication (so --help works without display)
    Args args = parseArgs(argc, argv);
    
    if (args.showHelp) {
        printHelp();
        return 0;
    }
    
    // Set up Qt for Wayland
    qputenv("QT_WAYLAND_FORCE_DPI", "96");
    
    // Create QApplication for dialog support
    QApplication app(argc, argv);
    app.setApplicationName("omnirec-picker");
    app.setDesktopFileName("omnirec-picker");
    
    if (args.dryRun) {
        return runDryRun(args.sourceType, args.sourceId);
    }
    
    return runPicker();
}
