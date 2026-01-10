//! Input validation for IPC request parameters.

use once_cell::sync::Lazy;
use regex::Regex;

/// Monitor ID pattern that accepts:
/// - Linux/Wayland: alphanumeric, dash, underscore (e.g., "DP-1", "HDMI-A-1", "eDP-1")
/// - Windows: device paths like "\\.\DISPLAY1" (backslash, dot, alphanumeric)
/// - macOS: numeric display IDs
static MONITOR_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\\\./A-Za-z0-9_\-]{1,64}$").unwrap());

/// Audio source ID pattern that accepts:
/// - Linux/PipeWire: dots, colons (e.g., "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor")
/// - Windows WASAPI: endpoint IDs with curly braces and pipes (e.g., "{0.0.0.00000000}.{guid}")
static SOURCE_ID_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9_.\-:\{\}#|]{1,256}$").unwrap());

/// Maximum coordinate value (positive or negative)
pub const MAX_COORDINATE: i32 = 65535;

/// Maximum dimension value (must be positive)
pub const MAX_DIMENSION: u32 = 16384;

/// Validation error types.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Monitor ID contains invalid characters or is too long
    InvalidMonitorId(String),
    /// Audio source ID contains invalid characters or is too long
    InvalidSourceId(String),
    /// Window handle is invalid (negative on some platforms)
    InvalidWindowHandle(isize),
    /// Dimension (width/height) is out of valid range
    DimensionOutOfRange {
        field: &'static str,
        value: u32,
        max: u32,
    },
    /// Coordinate (x/y) is out of valid range
    CoordinateOutOfRange { field: &'static str, value: i32 },
    /// String field exceeds maximum length
    StringTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
    /// Message exceeds maximum size
    MessageTooLarge { size: usize, max: usize },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidMonitorId(id) => write!(f, "Invalid monitor ID: {}", id),
            ValidationError::InvalidSourceId(id) => write!(f, "Invalid source ID: {}", id),
            ValidationError::InvalidWindowHandle(h) => write!(f, "Invalid window handle: {}", h),
            ValidationError::DimensionOutOfRange { field, value, max } => {
                write!(f, "{} out of range: {} (max {})", field, value, max)
            }
            ValidationError::CoordinateOutOfRange { field, value } => {
                write!(f, "{} out of range: {}", field, value)
            }
            ValidationError::StringTooLong { field, len, max } => {
                write!(f, "{} too long: {} chars (max {})", field, len, max)
            }
            ValidationError::MessageTooLarge { size, max } => {
                write!(f, "Message too large: {} bytes (max {})", size, max)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a monitor ID string.
///
/// Monitor IDs must match the pattern: alphanumeric, dash, underscore, 1-64 chars.
/// Examples: "DP-1", "HDMI-A-1", "eDP-1", "Virtual1"
pub fn validate_monitor_id(id: &str) -> Result<(), ValidationError> {
    if !MONITOR_ID_PATTERN.is_match(id) {
        return Err(ValidationError::InvalidMonitorId(id.to_string()));
    }
    Ok(())
}

/// Validate an audio source ID string.
///
/// Source IDs can contain alphanumeric, dots, colons, dashes, underscores, 1-256 chars.
/// Examples: "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"
pub fn validate_source_id(id: &str) -> Result<(), ValidationError> {
    if !SOURCE_ID_PATTERN.is_match(id) {
        return Err(ValidationError::InvalidSourceId(id.to_string()));
    }
    Ok(())
}

/// Validate a window handle.
///
/// Window handles are platform-specific integers. On most platforms,
/// valid handles are non-negative.
pub fn validate_window_handle(handle: isize) -> Result<(), ValidationError> {
    // On most platforms, window handles should be non-negative
    // Windows HWND can technically be any value, but negative is unusual
    if handle < 0 {
        return Err(ValidationError::InvalidWindowHandle(handle));
    }
    Ok(())
}

/// Validate dimension values (width, height).
///
/// Dimensions must be positive and not exceed MAX_DIMENSION (16384).
pub fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0 || width > MAX_DIMENSION {
        return Err(ValidationError::DimensionOutOfRange {
            field: "width",
            value: width,
            max: MAX_DIMENSION,
        });
    }
    if height == 0 || height > MAX_DIMENSION {
        return Err(ValidationError::DimensionOutOfRange {
            field: "height",
            value: height,
            max: MAX_DIMENSION,
        });
    }
    Ok(())
}

/// Validate coordinate values (x, y).
///
/// Coordinates can be negative (for multi-monitor setups) but must be
/// within reasonable bounds (±MAX_COORDINATE).
pub fn validate_coordinates(x: i32, y: i32) -> Result<(), ValidationError> {
    if !(-MAX_COORDINATE..=MAX_COORDINATE).contains(&x) {
        return Err(ValidationError::CoordinateOutOfRange {
            field: "x",
            value: x,
        });
    }
    if !(-MAX_COORDINATE..=MAX_COORDINATE).contains(&y) {
        return Err(ValidationError::CoordinateOutOfRange {
            field: "y",
            value: y,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_monitor_ids() {
        // Linux/Wayland style
        assert!(validate_monitor_id("DP-1").is_ok());
        assert!(validate_monitor_id("HDMI-A-1").is_ok());
        assert!(validate_monitor_id("eDP-1").is_ok());
        assert!(validate_monitor_id("Virtual1").is_ok());
        assert!(validate_monitor_id("DVI_D_0").is_ok());
        // Windows style
        assert!(validate_monitor_id(r"\\.\DISPLAY1").is_ok());
        assert!(validate_monitor_id(r"\\.\DISPLAY2").is_ok());
    }

    #[test]
    fn test_invalid_monitor_ids() {
        // Empty
        assert!(validate_monitor_id("").is_err());
        // Too long
        assert!(validate_monitor_id(&"a".repeat(65)).is_err());
        // Contains spaces
        assert!(validate_monitor_id("DP 1").is_err());
    }

    #[test]
    fn test_valid_source_ids() {
        // Linux/PipeWire style
        assert!(validate_source_id("123").is_ok());
        assert!(validate_source_id("alsa_output.pci-0000_00_1f.3.analog-stereo.monitor").is_ok());
        assert!(validate_source_id("pulse:sink:0").is_ok());
        // Windows WASAPI style endpoint IDs
        assert!(
            validate_source_id("{0.0.0.00000000}.{b3f8fa53-0004-438e-9003-51a46e139bfc}").is_ok()
        );
        assert!(
            validate_source_id("{0.0.1.00000000}.{d11a3f67-7b3e-4c7e-b123-456789abcdef}").is_ok()
        );
    }

    #[test]
    fn test_invalid_source_ids() {
        assert!(validate_source_id("").is_err());
        assert!(validate_source_id(&"a".repeat(257)).is_err());
        assert!(validate_source_id("path/to/device").is_err());
    }

    #[test]
    fn test_dimensions() {
        assert!(validate_dimensions(1920, 1080).is_ok());
        assert!(validate_dimensions(1, 1).is_ok());
        assert!(validate_dimensions(MAX_DIMENSION, MAX_DIMENSION).is_ok());

        assert!(validate_dimensions(0, 1080).is_err());
        assert!(validate_dimensions(1920, 0).is_err());
        assert!(validate_dimensions(MAX_DIMENSION + 1, 1080).is_err());
    }

    #[test]
    fn test_coordinates() {
        assert!(validate_coordinates(0, 0).is_ok());
        assert!(validate_coordinates(-1920, 0).is_ok());
        assert!(validate_coordinates(3840, 2160).is_ok());

        assert!(validate_coordinates(MAX_COORDINATE + 1, 0).is_err());
        assert!(validate_coordinates(0, -MAX_COORDINATE - 1).is_err());
    }
}
