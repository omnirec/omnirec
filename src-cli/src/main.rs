//! OmniRec Command-Line Interface
//!
//! A headless CLI for screen recording, enabling scriptable automation
//! and remote workflows without requiring the GUI.

mod client;
mod colors;
mod commands;
mod exit_codes;
mod platform;

use clap::{Parser, Subcommand};
use exit_codes::ExitCode;

/// OmniRec - Screen Recording CLI
#[derive(Parser, Debug)]
#[command(name = "omnirec")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output in JSON format for scripting
    #[arg(long, global = true)]
    json: bool,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List available capture sources
    List {
        #[command(subcommand)]
        source: ListSource,
    },
    /// Start a recording
    Record {
        #[command(subcommand)]
        target: RecordTarget,
    },
    /// Stop the current recording
    Stop,
    /// Show current recording status
    Status,
    /// Show version information
    Version,
}

#[derive(Subcommand, Debug)]
enum ListSource {
    /// List capturable windows
    Windows,
    /// List available displays/monitors
    Displays,
    /// List audio sources (system audio and microphones)
    Audio,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RecordTarget {
    /// Record a specific window by handle
    Window {
        /// Window handle (use 'omnirec list windows' to find)
        #[arg(allow_hyphen_values = true)]
        handle: isize,

        #[command(flatten)]
        options: RecordOptions,
    },
    /// Record a specific display by ID
    Display {
        /// Display ID (use 'omnirec list displays' to find)
        id: String,

        #[command(flatten)]
        options: RecordOptions,
    },
    /// Record a specific screen region
    Region {
        /// Display ID for the region
        #[arg(long)]
        display: String,

        /// X coordinate (pixels)
        #[arg(long)]
        x: i32,

        /// Y coordinate (pixels)
        #[arg(long)]
        y: i32,

        /// Width (pixels)
        #[arg(long)]
        width: u32,

        /// Height (pixels)
        #[arg(long)]
        height: u32,

        #[command(flatten)]
        options: RecordOptions,
    },
    /// Record using the desktop portal picker (Wayland)
    Portal {
        #[command(flatten)]
        options: RecordOptions,
    },
}

#[derive(Parser, Debug, Clone)]
pub struct RecordOptions {
    /// Output file path (overrides configured output directory)
    #[arg(short, long)]
    output: Option<String>,

    /// Output format: mp4, webm, mkv, mov, gif, apng, webp
    #[arg(short, long, default_value = "mp4")]
    format: String,

    /// Auto-stop after duration (seconds)
    #[arg(short, long)]
    duration: Option<u64>,

    /// System audio source ID (use 'none' to disable)
    #[arg(long)]
    audio: Option<String>,

    /// Microphone source ID (use 'none' to disable)
    #[arg(long)]
    microphone: Option<String>,

    /// Fail if specific target cannot be selected (don't fall back to portal)
    #[arg(long)]
    strict: bool,
}

fn main() {
    let cli = Cli::parse();

    // Build the async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let exit_code = runtime.block_on(run(cli));
    std::process::exit(exit_code.as_i32());
}

async fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Commands::List { source } => match source {
            ListSource::Windows => commands::list_windows(cli.json, cli.quiet).await,
            ListSource::Displays => commands::list_displays(cli.json, cli.quiet).await,
            ListSource::Audio => commands::list_audio(cli.json, cli.quiet).await,
        },
        Commands::Record { target } => {
            commands::record(target, cli.json, cli.quiet, cli.verbose).await
        }
        Commands::Stop => commands::stop(cli.json, cli.quiet).await,
        Commands::Status => commands::status(cli.json).await,
        Commands::Version => {
            commands::version(cli.json);
            ExitCode::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Verify the CLI definition is valid
    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    /// Test parsing 'list windows' command
    #[test]
    fn parse_list_windows() {
        let cli = Cli::try_parse_from(["omnirec", "list", "windows"]).unwrap();
        assert!(!cli.json);
        assert!(!cli.quiet);
        assert!(!cli.verbose);
        assert!(matches!(
            cli.command,
            Commands::List {
                source: ListSource::Windows
            }
        ));
    }

    /// Test parsing 'list displays' command
    #[test]
    fn parse_list_displays() {
        let cli = Cli::try_parse_from(["omnirec", "list", "displays"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::List {
                source: ListSource::Displays
            }
        ));
    }

    /// Test parsing 'list audio' command
    #[test]
    fn parse_list_audio() {
        let cli = Cli::try_parse_from(["omnirec", "list", "audio"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::List {
                source: ListSource::Audio
            }
        ));
    }

    /// Test parsing list command with --json flag
    #[test]
    fn parse_list_with_json() {
        let cli = Cli::try_parse_from(["omnirec", "--json", "list", "windows"]).unwrap();
        assert!(cli.json);
        assert!(!cli.quiet);
    }

    /// Test parsing list command with --quiet flag
    #[test]
    fn parse_list_with_quiet() {
        let cli = Cli::try_parse_from(["omnirec", "-q", "list", "displays"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.json);
    }

    /// Test parsing 'record window' command
    #[test]
    fn parse_record_window() {
        let cli = Cli::try_parse_from(["omnirec", "record", "window", "12345"]).unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Window { handle, options },
            } => {
                assert_eq!(handle, 12345);
                assert_eq!(options.format, "mp4");
                assert!(options.output.is_none());
                assert!(options.duration.is_none());
                assert!(!options.strict);
            }
            _ => panic!("Expected Record Window command"),
        }
    }

    /// Test parsing 'record display' command
    #[test]
    fn parse_record_display() {
        let cli = Cli::try_parse_from(["omnirec", "record", "display", "HDMI-1"]).unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Display { id, .. },
            } => {
                assert_eq!(id, "HDMI-1");
            }
            _ => panic!("Expected Record Display command"),
        }
    }

    /// Test parsing 'record region' command with all required flags
    #[test]
    fn parse_record_region() {
        let cli = Cli::try_parse_from([
            "omnirec",
            "record",
            "region",
            "--display",
            "DP-1",
            "--x",
            "100",
            "--y",
            "200",
            "--width",
            "800",
            "--height",
            "600",
        ])
        .unwrap();
        match cli.command {
            Commands::Record {
                target:
                    RecordTarget::Region {
                        display,
                        x,
                        y,
                        width,
                        height,
                        ..
                    },
            } => {
                assert_eq!(display, "DP-1");
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(width, 800);
                assert_eq!(height, 600);
            }
            _ => panic!("Expected Record Region command"),
        }
    }

    /// Test parsing 'record portal' command
    #[test]
    fn parse_record_portal() {
        let cli = Cli::try_parse_from(["omnirec", "record", "portal"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Record {
                target: RecordTarget::Portal { .. }
            }
        ));
    }

    /// Test parsing record command with output options
    #[test]
    fn parse_record_with_options() {
        let cli = Cli::try_parse_from([
            "omnirec",
            "record",
            "display",
            "0",
            "-o",
            "/tmp/recording.webm",
            "-f",
            "webm",
            "-d",
            "60",
        ])
        .unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Display { options, .. },
            } => {
                assert_eq!(options.output, Some("/tmp/recording.webm".to_string()));
                assert_eq!(options.format, "webm");
                assert_eq!(options.duration, Some(60));
            }
            _ => panic!("Expected Record Display command"),
        }
    }

    /// Test parsing record command with audio options
    #[test]
    fn parse_record_with_audio() {
        let cli = Cli::try_parse_from([
            "omnirec",
            "record",
            "display",
            "0",
            "--audio",
            "default",
            "--microphone",
            "none",
        ])
        .unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Display { options, .. },
            } => {
                assert_eq!(options.audio, Some("default".to_string()));
                assert_eq!(options.microphone, Some("none".to_string()));
            }
            _ => panic!("Expected Record Display command"),
        }
    }

    /// Test parsing record command with --strict flag
    #[test]
    fn parse_record_with_strict() {
        let cli = Cli::try_parse_from(["omnirec", "record", "window", "123", "--strict"]).unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Window { options, .. },
            } => {
                assert!(options.strict);
            }
            _ => panic!("Expected Record Window command"),
        }
    }

    /// Test parsing 'stop' command
    #[test]
    fn parse_stop() {
        let cli = Cli::try_parse_from(["omnirec", "stop"]).unwrap();
        assert!(matches!(cli.command, Commands::Stop));
    }

    /// Test parsing 'status' command
    #[test]
    fn parse_status() {
        let cli = Cli::try_parse_from(["omnirec", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    /// Test parsing 'version' command
    #[test]
    fn parse_version() {
        let cli = Cli::try_parse_from(["omnirec", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version));
    }

    /// Test that global flags work after subcommand
    #[test]
    fn parse_global_flags_after_subcommand() {
        let cli = Cli::try_parse_from(["omnirec", "list", "windows", "--json", "-q"]).unwrap();
        assert!(cli.json);
        assert!(cli.quiet);
    }

    /// Test invalid command returns error
    #[test]
    fn parse_invalid_command() {
        let result = Cli::try_parse_from(["omnirec", "invalid"]);
        assert!(result.is_err());
    }

    /// Test missing required argument returns error
    #[test]
    fn parse_missing_window_handle() {
        let result = Cli::try_parse_from(["omnirec", "record", "window"]);
        assert!(result.is_err());
    }

    /// Test missing required region flags returns error
    #[test]
    fn parse_missing_region_flags() {
        let result = Cli::try_parse_from(["omnirec", "record", "region", "--display", "0"]);
        assert!(result.is_err());
    }

    /// Test negative window handle (valid for some platforms)
    #[test]
    fn parse_negative_window_handle() {
        let cli = Cli::try_parse_from(["omnirec", "record", "window", "-1"]).unwrap();
        match cli.command {
            Commands::Record {
                target: RecordTarget::Window { handle, .. },
            } => {
                assert_eq!(handle, -1);
            }
            _ => panic!("Expected Record Window command"),
        }
    }
}
