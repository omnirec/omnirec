## MODIFIED Requirements

### Requirement: Monitor Enumeration Trait

The system SHALL define a `MonitorEnumerator` trait that abstracts platform-specific monitor listing, returning a consistent `MonitorInfo` structure across all platforms.

#### Scenario: List available monitors

- **GIVEN** a platform-specific backend implementing `MonitorEnumerator`
- **WHEN** `list_monitors` is called
- **THEN** a list of `MonitorInfo` structs is returned
- **AND** each struct contains ID, name, position, dimensions, and primary flag

#### Scenario: Primary monitor ordering

- **GIVEN** a platform-specific backend implementing `MonitorEnumerator`
- **WHEN** `list_monitors` is called on a multi-monitor system
- **THEN** the primary monitor appears first in the returned list

#### Scenario: Windows DPI scale factor detection

- **GIVEN** the application is running on Windows
- **WHEN** `list_monitors` is called
- **THEN** each `MonitorInfo.scale_factor` reflects the actual Windows DPI scaling
- **AND** a 100% scaled monitor returns `scale_factor: 1.0`
- **AND** a 125% scaled monitor returns `scale_factor: 1.25`
- **AND** a 150% scaled monitor returns `scale_factor: 1.5`
- **AND** a 200% scaled monitor returns `scale_factor: 2.0`
