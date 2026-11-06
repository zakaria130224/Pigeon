//! The few command line options.

use pigeon_core::{APP_NAME, VERSION};
use std::path::PathBuf;

const HELP: &str = "Usage: pigeon [options]

Options:
  --screenshot <file.png>  Open normally, draw about 120 frames with sample data,
                           save a picture of the window and exit
  --version                Show the version
  --help                   Show this help";

#[derive(Debug, PartialEq)]
pub enum Command {
    Run { screenshot: Option<PathBuf> },
    Print(String),
}

/// Unknown options are ignored: desktops sometimes pass their own when starting an app.
pub fn parse(args: impl Iterator<Item = String>) -> Command {
    let mut screenshot = None;
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => return Command::Print(format!("{APP_NAME} {VERSION}")),
            "--help" | "-h" => return Command::Print(HELP.to_string()),
            "--screenshot" => screenshot = args.next().map(PathBuf::from),
            other => {
                if let Some(path) = other.strip_prefix("--screenshot=") {
                    screenshot = Some(PathBuf::from(path));
                }
            }
        }
    }
    Command::Run { screenshot }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Command {
        parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn reads_options() {
        assert_eq!(parsed(&[]), Command::Run { screenshot: None });
        assert_eq!(parsed(&["--screenshot", "a.png"]), Command::Run { screenshot: Some("a.png".into()) });
        assert_eq!(parsed(&["--screenshot=b.png"]), Command::Run { screenshot: Some("b.png".into()) });
        assert_eq!(parsed(&["-psn_0_12345"]), Command::Run { screenshot: None });
        assert_eq!(parsed(&["--version"]), Command::Print(format!("Pigeon {VERSION}")));
        assert!(matches!(parsed(&["--help"]), Command::Print(text) if text.contains("--screenshot")));
    }
}
