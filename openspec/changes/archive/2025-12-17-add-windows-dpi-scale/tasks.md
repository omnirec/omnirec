## 1. Implementation

- [x] 1.1 Add `GetDpiForMonitor` import from `windows::Win32::UI::HiDpi` module
- [x] 1.2 Call `GetDpiForMonitor` in `enum_monitor_callback` to retrieve monitor DPI
- [x] 1.3 Calculate scale factor as `dpi / 96.0` (96 DPI is the Windows baseline)
- [x] 1.4 Replace hardcoded `1.0` with calculated scale factor

## 2. Testing

- [x] 2.1 Verify compilation on Windows with new API imports
- [x] 2.2 Run existing `monitor_list` unit tests
- [x] 2.3 Manual test: Verify scale factor is correct on 100%, 125%, 150%, 200% displays

## 3. Validation

- [x] 3.1 Run `cargo clippy` to ensure no warnings
- [x] 3.2 Test region capture coordinates on high-DPI display
