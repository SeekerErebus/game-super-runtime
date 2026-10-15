use anyhow::{Context, Result, bail, ensure};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Launch,
    Edit,
    Gui,
    Validate,
    DryRun,
    Doctor,
    ConfigPath,
    Logs,
}
#[derive(Debug)]
pub struct Cli {
    pub config_dir: Option<PathBuf>,
    pub id: Option<String>,
    pub mode: Mode,
    pub command: Vec<OsString>,
    pub no_open: bool,
    pub global: Option<String>,
}
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Cli> {
    let args: Vec<_> = args.into_iter().collect();
    let mut c = Cli {
        config_dir: None,
        id: None,
        mode: Mode::Launch,
        command: vec![],
        no_open: false,
        global: None,
    };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].to_str();
        match arg {
            Some("--config-dir") => {
                i += 1;
                c.config_dir = Some(PathBuf::from(
                    args.get(i).context("--config-dir needs a directory")?,
                ));
            }
            Some(
                "--help"
                | "-h"
                | "--version"
                | "--list"
                | "--edit-defaults"
                | "--edit-defaults-gui",
            ) if c.id.is_none() => {
                c.global = arg.map(str::to_owned);
            }
            Some("--no-open") => c.no_open = true,
            Some(
                "--edit" | "--edit-gui" | "--validate" | "--dry-run" | "--doctor" | "--config-path"
                | "--logs",
            ) if c.id.is_some() => {
                ensure!(c.mode == Mode::Launch, "choose only one action flag");
                c.mode = match arg.unwrap() {
                    "--edit" => Mode::Edit,
                    "--edit-gui" => Mode::Gui,
                    "--validate" => Mode::Validate,
                    "--dry-run" => Mode::DryRun,
                    "--doctor" => Mode::Doctor,
                    "--config-path" => Mode::ConfigPath,
                    _ => Mode::Logs,
                };
            }
            Some("--") if c.id.is_some() => {
                c.command = args[i + 1..].to_vec();
                break;
            }
            Some(a) if c.id.is_none() && !a.starts_with('-') => {
                c.id = Some(a.into());
            }
            _ if c.id.is_some() => {
                if arg.is_some_and(|a| a.starts_with('-')) {
                    bail!(
                        "unknown option {}; put -- before a command whose name begins with '-'",
                        args[i].to_string_lossy()
                    );
                }
                c.command = args[i..].to_vec();
                break;
            }
            _ => bail!("unknown option {}; use --help", args[i].to_string_lossy()),
        }
        i += 1;
    }
    ensure!(
        c.global.is_none() || c.id.is_none(),
        "global actions cannot be combined with APP-ID"
    );
    ensure!(
        c.command.is_empty() || matches!(c.mode, Mode::Launch | Mode::DryRun | Mode::Doctor),
        "this action does not accept a launch command"
    );
    if let Some(id) = &c.id {
        crate::config::valid_id(id)?;
    }
    Ok(c)
}

pub const HELP: &str = "Game Super Runtime — declarative Linux game launches

Usage:
  game-super-runtime [--config-dir DIR] APP-ID [OPTIONS] [--] [COMMAND...]
  game-super-runtime --edit-defaults [--config-dir DIR]
  game-super-runtime --edit-defaults-gui [--no-open]

Steam launch options:
  game-super-runtime 1091500 %command%

Actions (before COMMAND; all remaining arguments belong to the game):
  --edit         Create/open APP-ID.yaml in VISUAL, EDITOR or the default editor
  --edit-gui     Open the local graphical configuration editor in your browser
  --no-open      Print the GUI URL without opening a browser
  --validate     Validate YAML without launching anything
  --dry-run      Print the resolved plan as JSON (may contain private values)
  --doctor       Validate the launch plan and check host executables
  --config-path  Print the configuration file location
  --logs         Print this app's log directory
  --list         List configured apps
  --help, -h     Show this help
  --version      Show version

Configuration: $GSR_CONFIG_DIR or $XDG_CONFIG_HOME/game-super-runtime
               (default ~/.config/game-super-runtime)
Logs:          $XDG_STATE_HOME/game-super-runtime/logs/APP-ID
               (default ~/.local/state/game-super-runtime/logs/APP-ID)
New apps copy defaults.yaml once. Existing configs are never silently overwritten.
Commands are argument lists, not shell strings. See docs/reference.md.
";
