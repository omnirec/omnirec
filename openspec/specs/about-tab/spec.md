# about-tab Specification

## Purpose
TBD - created by archiving change add-gnome-desktop-support. Update Purpose after archive.
## Requirements
### Requirement: About Tab Access

The application SHALL provide an About tab accessible from the main capture mode tab bar on all platforms.

#### Scenario: About button displayed in tab bar

- **WHEN** the application window is displayed
- **THEN** an info icon button SHALL be visible on the right side of the tab bar
- **AND** the button SHALL be positioned after the Configuration (gear) button
- **AND** the button SHALL be visually styled as an info/about action

#### Scenario: About tab activation

- **WHEN** the user clicks the info icon button
- **THEN** the About view SHALL be displayed
- **AND** the capture mode tabs (Window, Region, Display) SHALL appear inactive
- **AND** the Configuration button SHALL appear inactive
- **AND** the About button SHALL appear active/selected

#### Scenario: Return to capture mode from About

- **WHEN** the About view is active
- **AND** the user clicks any capture mode tab (Window, Region, Display)
- **THEN** the corresponding capture view SHALL be displayed
- **AND** the About button SHALL appear inactive

#### Scenario: About tab always accessible

- **WHEN** the application is in idle state (not recording)
- **THEN** the About tab SHALL be accessible regardless of platform

#### Scenario: About tab disabled during recording

- **WHEN** a recording is in progress
- **THEN** the About button SHALL be disabled
- **AND** the user SHALL NOT be able to switch to the About view

### Requirement: About View Content

The About view SHALL display application information including version, links, and legal notices.

#### Scenario: Version displayed

- **WHEN** the About view is active
- **THEN** the application version number SHALL be displayed
- **AND** the version SHALL match the version shown in the header

#### Scenario: Website link displayed

- **WHEN** the About view is active
- **THEN** a link to the OmniRec website SHALL be displayed
- **AND** clicking the link SHALL open the website in the default browser

#### Scenario: GitHub link displayed

- **WHEN** the About view is active
- **THEN** a link to the OmniRec GitHub repository SHALL be displayed
- **AND** clicking the link SHALL open the repository in the default browser

#### Scenario: Copyright notice displayed

- **WHEN** the About view is active
- **THEN** a copyright notice SHALL be displayed
- **AND** the notice SHALL include the current year

#### Scenario: License information displayed

- **WHEN** the About view is active
- **THEN** the software license name SHALL be displayed
- **AND** a link to the full license text SHALL be available

### Requirement: About View Layout

The About view SHALL have a clean, centered layout appropriate for informational content.

#### Scenario: Centered content layout

- **WHEN** the About view is active
- **THEN** the content SHALL be centered within the view
- **AND** the layout SHALL be visually distinct from the configuration settings

#### Scenario: Application branding

- **WHEN** the About view is active
- **THEN** the OmniRec logo or icon SHALL be displayed prominently
- **AND** the application name SHALL be displayed

#### Scenario: Visual hierarchy

- **WHEN** the About view is active
- **THEN** the version SHALL be prominently displayed near the app name
- **AND** links SHALL be grouped together
- **AND** legal text (copyright, license) SHALL be at the bottom

