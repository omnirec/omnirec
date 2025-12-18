## 1. Configuration

- [x] 1.1 Add `theme` field to `AppConfig` in `config.rs` with values: `auto`, `light`, `dark` (default: `auto`)
- [x] 1.2 Add Tauri command to get/set theme preference
- [x] 1.3 Add "Appearance" settings group to configuration view HTML
- [x] 1.4 Add theme mode selector dropdown to configuration view
- [x] 1.5 Wire up theme selector to save config on change

## 2. CSS Theme Variables

- [x] 2.1 Refactor CSS to use semantic theme variables that can be switched
- [x] 2.2 Create light mode color palette with near-white to cool gray gradient
- [x] 2.3 Define light mode values for all semantic color variables (background, text, borders, surfaces)
- [x] 2.4 Ensure control colors (buttons, inputs, dropdowns) have appropriate light mode styling

## 3. Theme Switching Logic

- [x] 3.1 Add `prefers-color-scheme` media query detection in TypeScript
- [x] 3.2 Implement `applyTheme()` function to switch CSS class on root element
- [x] 3.3 Apply theme on app startup based on config and system preference
- [x] 3.4 Listen for system theme changes when in auto mode
- [x] 3.5 Update theme immediately when user changes preference in settings

## 4. Testing and Validation

- [x] 4.1 Verify light mode gradient renders correctly
- [x] 4.2 Verify all UI elements have appropriate contrast in light mode
- [x] 4.3 Verify auto mode responds to system theme changes
- [x] 4.4 Verify manual mode overrides system preference
- [x] 4.5 Verify theme preference persists across app restarts
