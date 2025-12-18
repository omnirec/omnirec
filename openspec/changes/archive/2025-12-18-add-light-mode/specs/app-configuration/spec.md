## ADDED Requirements

### Requirement: Appearance Settings Group

The configuration view SHALL include an Appearance settings group for controlling visual preferences.

#### Scenario: Appearance group displayed

- **WHEN** the configuration view is active
- **THEN** an "Appearance" group section SHALL be visible
- **AND** the group SHALL contain the theme mode selector

#### Scenario: Appearance group position

- **WHEN** the configuration view is displayed
- **THEN** the Appearance group SHALL appear after the Audio group

### Requirement: Theme Mode Setting

The configuration view SHALL provide a selector to choose the application theme mode.

#### Scenario: Theme mode selector default state

- **WHEN** the application starts with no saved configuration
- **THEN** the theme mode selector SHALL display "Auto" as the default selection

#### Scenario: Theme mode options

- **WHEN** the user opens the theme mode selector
- **THEN** three options SHALL be available: "Auto", "Light", "Dark"

#### Scenario: Theme mode selection

- **WHEN** the user selects a theme mode from the dropdown
- **THEN** the selection SHALL be applied immediately
- **AND** the dropdown SHALL display the selected mode name
- **AND** the setting SHALL be saved to the configuration file automatically

#### Scenario: Theme mode selection persistence

- **WHEN** the application restarts
- **THEN** the previously selected theme mode SHALL be restored
- **AND** the corresponding theme SHALL be applied on startup

### Requirement: Theme Configuration Persistence

The application SHALL persist theme configuration alongside other settings.

#### Scenario: Theme config saved

- **WHEN** the user changes the theme mode setting
- **THEN** the setting SHALL be saved to the config file automatically
- **AND** the config file SHALL include the theme mode value

#### Scenario: Theme config loaded on startup

- **WHEN** the application starts
- **THEN** the theme configuration SHALL be loaded from the config file
- **AND** the UI SHALL reflect the loaded theme mode
- **AND** the appropriate theme SHALL be applied based on the mode and system preference
