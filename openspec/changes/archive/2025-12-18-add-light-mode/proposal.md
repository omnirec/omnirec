# Change: Add Light Mode Theme Support

## Why

Users on systems configured for light mode experience visual inconsistency with the dark-only UI. Adding light mode support improves accessibility and user experience by respecting system preferences.

## What Changes

- Add a new "Appearance" settings group in the configuration view with a theme mode selector (Auto, Light, Dark)
- Implement light mode CSS theme with a gradient background from near-white (top-left) to cool gray (bottom-right)
- Add system theme detection using `prefers-color-scheme` media query
- Auto mode (default) follows system preference; manual selection overrides it
- Persist theme preference in the app configuration file

## Impact

- Affected specs: `ui-theme`, `app-configuration`
- Affected code: `src/styles.css`, `src/main.ts`, `src-tauri/src/config.rs`
