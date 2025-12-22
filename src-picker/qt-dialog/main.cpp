/**
 * OmniRec Permission Dialog
 * 
 * A simple Qt6 dialog for screen recording permission.
 * 
 * Usage: omnirec-dialog <source_description>
 * 
 * Output (stdout):
 *   ALWAYS_ALLOW - User clicked "Always Allow"
 *   ALLOW_ONCE   - User clicked "Allow Once"  
 *   DENY         - User clicked "Deny" or closed the dialog
 * 
 * Exit codes:
 *   0 - User approved (ALWAYS_ALLOW or ALLOW_ONCE)
 *   1 - User denied
 */

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
#include <iostream>

enum class Result {
    AlwaysAllow,
    AllowOnce,
    Deny
};

class PermissionDialog : public QDialog {
public:
    PermissionDialog(const QString& sourceDesc, QWidget* parent = nullptr) 
        : QDialog(parent), result_(Result::Deny) 
    {
        setWindowTitle("OmniRec - Screen Recording Permission");
        setMinimumWidth(400);
        
        // Set window flags for floating behavior in tiling WMs:
        // - Tool: Utility window type, typically floated by compositors
        // - WindowStaysOnTopHint: Keep above other windows  
        // - CustomizeWindowHint + WindowCloseButtonHint: Keep close button
        setWindowFlags(Qt::Tool | Qt::WindowStaysOnTopHint | 
                       Qt::CustomizeWindowHint | Qt::WindowTitleHint | Qt::WindowCloseButtonHint);
        
        // Set window modality to make it behave as a modal dialog
        setWindowModality(Qt::ApplicationModal);
        
        // Fixed size prevents resizing and helps with floating
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
            result_ = Result::Deny;
            reject();
        });
        
        auto* allowOnceBtn = new QPushButton("Allow Once", this);
        allowOnceBtn->setMinimumWidth(100);
        connect(allowOnceBtn, &QPushButton::clicked, this, [this]() {
            result_ = Result::AllowOnce;
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
            result_ = Result::AlwaysAllow;
            accept();
        });
        
        buttonLayout->addStretch();
        buttonLayout->addWidget(denyBtn);
        buttonLayout->addWidget(allowOnceBtn);
        buttonLayout->addWidget(alwaysAllowBtn);
        
        layout->addLayout(buttonLayout);
    }
    
    Result result() const { return result_; }
    
private:
    Result result_;
};

int main(int argc, char* argv[]) {
    // Set desktop file name for proper Wayland app_id
    // This MUST be set before QApplication is created
    qputenv("QT_WAYLAND_FORCE_DPI", "96");
    
    QApplication app(argc, argv);
    app.setApplicationName("omnirec-dialog");
    app.setDesktopFileName("omnirec-dialog");
    
    QString sourceDesc = "Screen recording requested";
    if (argc > 1) {
        sourceDesc = QString::fromUtf8(argv[1]);
    }
    
    PermissionDialog dialog(sourceDesc);
    
    // Set object name for window matching (used by some compositors)
    dialog.setObjectName("omnirec-dialog");
    
    // Center on screen
    dialog.adjustSize();
    if (QScreen* screen = app.primaryScreen()) {
        QRect screenGeometry = screen->availableGeometry();
        int x = (screenGeometry.width() - dialog.width()) / 2 + screenGeometry.x();
        int y = (screenGeometry.height() - dialog.height()) / 2 + screenGeometry.y();
        dialog.move(x, y);
    }
    
    dialog.exec();
    
    switch (dialog.result()) {
        case Result::AlwaysAllow:
            std::cout << "ALWAYS_ALLOW" << std::endl;
            return 0;
        case Result::AllowOnce:
            std::cout << "ALLOW_ONCE" << std::endl;
            return 0;
        case Result::Deny:
        default:
            std::cout << "DENY" << std::endl;
            return 1;
    }
}
