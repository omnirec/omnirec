/**
 * Approval dialog for screen recording consent.
 *
 * This module provides a Qt6 dialog that asks the user to approve
 * OmniRec's screen recording request. If Qt fails, it falls back
 * to hyprland-dialog.
 */

#ifndef DIALOG_H
#define DIALOG_H

#include <QString>

/**
 * Result of the approval dialog.
 */
enum class DialogResult {
    /// User approved with "Always Allow"
    AlwaysAllow,
    /// User approved with "Allow Once"
    AllowOnce,
    /// User denied the request
    Denied
};

/**
 * Show the approval dialog and wait for user response.
 *
 * Uses our embedded Qt6 dialog for a polished UI.
 * Falls back to hyprland-dialog if Qt fails.
 *
 * @param sourceType Type of capture source (e.g., "monitor", "window", "region")
 * @param sourceId Identifier of the capture source (e.g., "DP-1", window title)
 * @return The dialog result
 */
DialogResult showApprovalDialog(const QString& sourceType, const QString& sourceId);

/**
 * Generate a random 256-bit approval token as a hex string.
 * @return 64-character hex string
 */
QString generateApprovalToken();

#endif // DIALOG_H
