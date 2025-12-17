# Change: Add Windows DPI Scale Factor Detection

## Why

Windows monitor enumeration currently hardcodes `scale_factor: 1.0` for all monitors. This causes incorrect coordinate calculations on high-DPI displays (125%, 150%, 200% scaling), affecting region capture accuracy and display information in the UI.

## What Changes

- Implement DPI scale detection using the Windows `GetDpiForMonitor` API
- Replace hardcoded `1.0` with actual scale factor from the system
- No **BREAKING** changes - this is a bug fix that corrects existing behavior

## Impact

- Affected specs: `platform-abstraction` (MonitorInfo consistency)
- Affected code: `src-tauri/src/capture/windows/monitor_list.rs`
- Estimated effort: ~20 lines
