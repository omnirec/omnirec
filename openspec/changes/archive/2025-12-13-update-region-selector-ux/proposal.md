# Change: Improve Region Selector UX

## Why

The region selector overlay has several usability issues:
1. The dimensions indicator is always visible, which can be distracting during normal use
2. The selector window remains open after recording completes, requiring manual dismissal
3. Region position/size is lost when the selector is closed, requiring users to re-select regions
4. The selector can become orphaned if the main window is closed

## What Changes

- **Dimensions auto-hide**: Show dimensions only briefly (1.5s) after move/resize, then fade out
- **Auto-close on recording complete**: Close the region selector window when recording finishes
- **Persist region geometry**: Remember region position/size when selector closes; restore when "Change Region" clicked
- **Close with main window**: Close the region selector if the main application window is closed

## Impact

- Affected specs: region-capture
- Affected code:
  - `src/selection-overlay.ts` - dimensions visibility logic
  - `src/selection-overlay.css` - fade animation for dimensions
  - `src/main.ts` - selector lifecycle management, geometry persistence, main window close handling
