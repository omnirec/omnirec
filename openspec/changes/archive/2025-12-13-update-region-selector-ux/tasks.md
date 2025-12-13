## 1. Dimensions Auto-Hide
- [x] ~~1.1 Add CSS transition/animation for dimensions fade-out~~ (removed - WebKitGTK rendering issue on transparent windows)
- [x] ~~1.2 Add state tracking for dimensions visibility timeout in selection-overlay.ts~~ (removed)
- [x] ~~1.3 Show dimensions on move/resize, auto-hide after 1.5 seconds~~ (removed)
- [x] ~~1.4 Cancel pending hide when new move/resize occurs~~ (removed)
- [x] 1.5 Remove dimensions from region selector (WebKitGTK on Wayland doesn't repaint transparent windows correctly)

## 2. Auto-Close on Recording Complete
- [x] 2.1 Close region selector window in stopRecording() after recording completes successfully
- [x] 2.2 Ensure region state is preserved when selector is auto-closed

## 3. Persist Region Geometry
- [x] 3.1 Store last known selector window position/size when selector closes (via event payload from selector)
- [x] 3.2 Restore stored position/size when opening selector via "Change Region"
- [x] 3.3 Fall back to default centered position if no stored geometry exists
- [x] 3.4 Add `move_region_selector` Tauri command to move window via Hyprland IPC (Wayland ignores app position hints)

## 4. Close with Main Window
- [x] 4.1 Add window close event listener in main.ts
- [x] 4.2 Close region selector window when main window closes
- [x] 4.3 Add `core:window:allow-destroy` permission to capabilities

## 5. Validation
- [x] 5.1 ~~Test dimensions auto-hide timing and animation~~ (feature removed)
- [x] 5.2 Test region persistence across selector close/reopen cycles
- [x] 5.3 Test auto-close on recording complete
- [x] 5.4 Test cleanup on main window close
