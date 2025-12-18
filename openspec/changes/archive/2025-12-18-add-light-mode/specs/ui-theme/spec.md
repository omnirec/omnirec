## MODIFIED Requirements

### Requirement: Dark Cool Gray Theme

The application SHALL display a dark cool gray gradient theme when dark mode is active.

#### Scenario: Dark gradient background applied

- **WHEN** dark mode is active (either by user preference or auto mode with system in dark)
- **THEN** the background SHALL show a cool gray gradient (dark gray tones)
- **AND** all text colors SHALL provide sufficient contrast for readability

#### Scenario: Component colors match dark theme

- **WHEN** dark mode is active
- **THEN** buttons, lists, and input areas SHALL use complementary dark gray tones
- **AND** accent colors SHALL be visible against the dark background

## ADDED Requirements

### Requirement: Light Cool Gray Theme

The application SHALL display a light cool gray gradient theme when light mode is active.

#### Scenario: Light gradient background applied

- **WHEN** light mode is active (either by user preference or auto mode with system in light)
- **THEN** the background SHALL show a gradient from near-white at the top-left to cool gray at the bottom-right
- **AND** all text colors SHALL provide sufficient contrast for readability

#### Scenario: Component colors match light theme

- **WHEN** light mode is active
- **THEN** buttons, lists, and input areas SHALL use complementary light gray tones
- **AND** accent colors SHALL be visible against the light background

### Requirement: System Theme Detection

The application SHALL detect the operating system's color scheme preference.

#### Scenario: System preference detected on startup

- **WHEN** the application starts with theme mode set to "auto"
- **THEN** the application SHALL query the system's color scheme preference
- **AND** apply the corresponding theme (light or dark)

#### Scenario: System preference changes while running

- **WHEN** the user changes their system color scheme preference
- **AND** the application theme mode is set to "auto"
- **THEN** the application SHALL immediately switch to match the new system preference

### Requirement: Theme Mode Override

The application SHALL allow users to manually override the system theme preference.

#### Scenario: Manual light mode selection

- **WHEN** the user selects "Light" from the theme mode selector
- **THEN** the light theme SHALL be applied regardless of system preference
- **AND** changes to system preference SHALL NOT affect the displayed theme

#### Scenario: Manual dark mode selection

- **WHEN** the user selects "Dark" from the theme mode selector
- **THEN** the dark theme SHALL be applied regardless of system preference
- **AND** changes to system preference SHALL NOT affect the displayed theme

#### Scenario: Return to auto mode

- **WHEN** the user selects "Auto" from the theme mode selector
- **THEN** the theme SHALL immediately match the current system preference
- **AND** future system preference changes SHALL be tracked
