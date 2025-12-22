/**
 * Approval dialog implementation.
 *
 * Shows a Qt6 dialog for screen recording permission, with fallback
 * to hyprland-dialog if needed.
 */

#include "dialog.h"

#include <QApplication>
#include <QDialog>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QIcon>
#include <QStyle>
#include <QFont>
#include <QSizePolicy>
#include <QScreen>
#include <QProcess>
#include <QRandomGenerator>
#include <iostream>

/**
 * Internal result enum for the dialog.
 */
enum class InternalResult {
    AlwaysAllow,
    AllowOnce,
    Deny
};

/**
 * Permission dialog widget.
 */
class PermissionDialog : public QDialog {
public:
    PermissionDialog(const QString& sourceDesc, QWidget* parent = nullptr) 
        : QDialog(parent), result_(InternalResult::Deny) 
    {
        setWindowTitle("OmniRec - Screen Recording Permission");
        setMinimumWidth(400);
        
        // Set window flags for floating behavior in tiling WMs:
        // Qt::Dialog is the standard dialog type that compositors float by default
        // Qt::WindowStaysOnTopHint keeps it above other windows
        setWindowFlags(Qt::Dialog | Qt::WindowStaysOnTopHint);
        
        // Set window modality to make it behave as a modal dialog
        setWindowModality(Qt::ApplicationModal);
        
        // Fixed size policy prevents tiling WMs from resizing
        setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
        
        auto* layout = new QVBoxLayout(this);
        layout->setSpacing(16);
        layout->setContentsMargins(24, 24, 24, 24);
        
        // Icon and title row
        auto* headerLayout = new QHBoxLayout();
        
        auto* iconLabel = new QLabel(this);
        QIcon warningIcon = style()->standardIcon(QStyle::SP_MessageBoxQuestion);
        iconLabel->setPixmap(warningIcon.pixmap(48, 48));
        headerLayout->addWidget(iconLabel);
        
        auto* titleLabel = new QLabel("Allow OmniRec to record your screen?", this);
        QFont titleFont = titleLabel->font();
        titleFont.setPointSize(titleFont.pointSize() + 2);
        titleFont.setBold(true);
        titleLabel->setFont(titleFont);
        titleLabel->setWordWrap(true);
        headerLayout->addWidget(titleLabel, 1);
        
        layout->addLayout(headerLayout);
        
        // Source description
        auto* descLabel = new QLabel(sourceDesc, this);
        descLabel->setWordWrap(true);
        descLabel->setStyleSheet("color: #666; padding-left: 64px;");
        layout->addWidget(descLabel);
        
        // Spacer
        layout->addSpacing(8);
        
        // Buttons
        auto* buttonLayout = new QHBoxLayout();
        buttonLayout->setSpacing(8);
        
        auto* denyBtn = new QPushButton("Deny", this);
        denyBtn->setMinimumWidth(100);
        connect(denyBtn, &QPushButton::clicked, this, [this]() {
            result_ = InternalResult::Deny;
            reject();
        });
        
        auto* allowOnceBtn = new QPushButton("Allow Once", this);
        allowOnceBtn->setMinimumWidth(100);
        connect(allowOnceBtn, &QPushButton::clicked, this, [this]() {
            result_ = InternalResult::AllowOnce;
            accept();
        });
        
        auto* alwaysAllowBtn = new QPushButton("Always Allow", this);
        alwaysAllowBtn->setMinimumWidth(120);
        alwaysAllowBtn->setDefault(true);
        alwaysAllowBtn->setStyleSheet(
            "QPushButton { background-color: #2196F3; color: white; font-weight: bold; }"
            "QPushButton:hover { background-color: #1976D2; }"
        );
        connect(alwaysAllowBtn, &QPushButton::clicked, this, [this]() {
            result_ = InternalResult::AlwaysAllow;
            accept();
        });
        
        buttonLayout->addStretch();
        buttonLayout->addWidget(denyBtn);
        buttonLayout->addWidget(allowOnceBtn);
        buttonLayout->addWidget(alwaysAllowBtn);
        
        layout->addLayout(buttonLayout);
    }
    
    InternalResult result() const { return result_; }
    
private:
    InternalResult result_;
};

/**
 * Format source description for display.
 */
static QString formatSourceDesc(const QString& sourceType, const QString& sourceId)
{
    if (sourceType == "monitor") {
        return QString("Display: %1").arg(sourceId);
    } else if (sourceType == "window") {
        return QString("Window: %1").arg(sourceId);
    } else if (sourceType == "region") {
        return QString("Region on: %1").arg(sourceId);
    } else {
        return QString("Source: %1").arg(sourceId);
    }
}

/**
 * Try to show dialog using hyprland-dialog (fallback).
 */
static DialogResult tryHyprlandDialog(const QString& sourceDesc)
{
    std::cerr << "[omnirec-picker] Trying hyprland-dialog (fallback)" << std::endl;
    
    QString text = QString("OmniRec is requesting permission to record your screen.\n\n%1").arg(sourceDesc);
    
    QProcess process;
    process.start("hyprland-dialog", QStringList() 
        << "--title" << "OmniRec - Screen Recording Permission"
        << "--text" << text
        << "--buttons" << "Always Allow;Allow Once;Deny");
    
    if (!process.waitForStarted(5000)) {
        std::cerr << "[omnirec-picker] hyprland-dialog not found" << std::endl;
        return DialogResult::Denied;
    }
    
    process.waitForFinished(-1);
    
    QString response = QString::fromUtf8(process.readAllStandardOutput()).trimmed();
    std::cerr << "[omnirec-picker] hyprland-dialog response: '" << response.toStdString() 
              << "' (exit: " << process.exitCode() << ")" << std::endl;
    
    if (response == "Always Allow") {
        std::cerr << "[omnirec-picker] User approved with always_allow=true" << std::endl;
        return DialogResult::AlwaysAllow;
    } else if (response == "Allow Once") {
        std::cerr << "[omnirec-picker] User approved with always_allow=false" << std::endl;
        return DialogResult::AllowOnce;
    } else {
        std::cerr << "[omnirec-picker] User denied" << std::endl;
        return DialogResult::Denied;
    }
}

DialogResult showApprovalDialog(const QString& sourceType, const QString& sourceId)
{
    std::cerr << "[omnirec-picker] show_approval_dialog called" << std::endl;
    
    QString sourceDesc = formatSourceDesc(sourceType, sourceId);
    
    // Check if we have a QApplication (we should, since main creates one)
    if (!QApplication::instance()) {
        std::cerr << "[omnirec-picker] No QApplication, falling back to hyprland-dialog" << std::endl;
        return tryHyprlandDialog(sourceDesc);
    }
    
    // Show our Qt dialog
    PermissionDialog dialog(sourceDesc);
    dialog.setObjectName("omnirec-dialog");
    
    // Center on screen and lock size to prevent tiling
    dialog.adjustSize();
    dialog.setFixedSize(dialog.size());  // Lock size after layout calculation
    
    if (QScreen* screen = QApplication::primaryScreen()) {
        QRect screenGeometry = screen->availableGeometry();
        int x = (screenGeometry.width() - dialog.width()) / 2 + screenGeometry.x();
        int y = (screenGeometry.height() - dialog.height()) / 2 + screenGeometry.y();
        dialog.move(x, y);
    }
    
    dialog.exec();
    
    switch (dialog.result()) {
        case InternalResult::AlwaysAllow:
            std::cerr << "[omnirec-picker] User approved with always_allow=true" << std::endl;
            return DialogResult::AlwaysAllow;
        case InternalResult::AllowOnce:
            std::cerr << "[omnirec-picker] User approved with always_allow=false" << std::endl;
            return DialogResult::AllowOnce;
        case InternalResult::Deny:
        default:
            std::cerr << "[omnirec-picker] User denied" << std::endl;
            return DialogResult::Denied;
    }
}

QString generateApprovalToken()
{
    // Generate 32 random bytes (256 bits)
    QByteArray bytes(32, 0);
    QRandomGenerator* rng = QRandomGenerator::global();
    for (int i = 0; i < 32; ++i) {
        bytes[i] = static_cast<char>(rng->bounded(256));
    }
    
    // Convert to hex string
    return bytes.toHex();
}
