//! Terminal color support for CLI output.
//!
//! Provides colorful output when running interactively, with automatic
//! detection to disable colors when output is piped or redirected.

use owo_colors::OwoColorize;
use std::io::IsTerminal;

/// Pad a string to a minimum width (left-aligned), then apply a color function.
/// This correctly handles ANSI escape codes by padding before colorizing.
pub fn pad_left<F>(msg: &str, width: usize, color_fn: F) -> String
where
    F: FnOnce(&str) -> String,
{
    let padded = format!("{:<width$}", msg);
    color_fn(&padded)
}

/// Check if stdout is a terminal (interactive mode).
pub fn is_interactive() -> bool {
    std::io::stdout().is_terminal()
}

/// Check if stderr is a terminal (interactive mode).
pub fn is_stderr_interactive() -> bool {
    std::io::stderr().is_terminal()
}

/// Style for error messages.
pub fn error(msg: &str) -> String {
    if is_stderr_interactive() {
        format!("{} {}", "error:".red().bold(), msg)
    } else {
        format!("error: {}", msg)
    }
}

/// Style for warning messages.
pub fn warning(msg: &str) -> String {
    if is_stderr_interactive() {
        format!("{} {}", "warning:".yellow().bold(), msg)
    } else {
        format!("warning: {}", msg)
    }
}

/// Style for success messages.
pub fn success(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.green())
    } else {
        msg.to_string()
    }
}

/// Style for info/status messages.
pub fn info(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.cyan())
    } else {
        msg.to_string()
    }
}

/// Style for dim/secondary text.
pub fn dim(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.dimmed())
    } else {
        msg.to_string()
    }
}

/// Style for bold text.
pub fn bold(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.bold())
    } else {
        msg.to_string()
    }
}

/// Style for header text (bold + color).
pub fn header(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.bold().blue())
    } else {
        msg.to_string()
    }
}

/// Style for recording indicator (red, pulsing effect simulated with bold).
pub fn recording(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.red().bold())
    } else {
        msg.to_string()
    }
}

/// Style for file paths.
pub fn path(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.underline())
    } else {
        msg.to_string()
    }
}

/// Style for numeric values (like handles, IDs).
pub fn number(msg: &str) -> String {
    if is_interactive() {
        format!("{}", msg.cyan())
    } else {
        msg.to_string()
    }
}

/// Style for "yes" indicator.
pub fn yes() -> String {
    if is_interactive() {
        format!("{}", "yes".green())
    } else {
        "yes".to_string()
    }
}

/// Style for "no" indicator.
pub fn no() -> String {
    if is_interactive() {
        format!("{}", "no".dimmed())
    } else {
        "no".to_string()
    }
}

/// Format elapsed time with color.
pub fn elapsed_time(mins: u64, secs: u64) -> String {
    let time_str = format!("{:02}:{:02}", mins, secs);
    if is_interactive() {
        format!("{}", time_str.yellow().bold())
    } else {
        time_str
    }
}

/// Format state name with appropriate color.
pub fn state(state: &str) -> String {
    if !is_interactive() {
        return state.to_string();
    }

    match state {
        "idle" => format!("{}", state.dimmed()),
        "recording" => format!("{}", state.red().bold()),
        "saving" => format!("{}", state.yellow()),
        _ => state.to_string(),
    }
}
