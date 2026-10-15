use anyhow::{Context, Result};
use game_super_runtime::{
    cli::{self, Mode},
    config, editor,
    execute::{self, Log},
    plan,
    storage::{self, Paths},
};
use std::env;

fn main() {
    let mut log = None;
    let result = run(&mut log);
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            let text = format!("{error:#}");
            eprintln!("game-super-runtime: {text}");
            if let Some(log) = log.as_mut() {
                log.write(&format!("ERROR: {text}"));
                eprintln!("Log: {}", log.path.display());
            }
            125
        }
    };
    std::process::exit(code);
}
fn run(log: &mut Option<Log>) -> Result<i32> {
    execute::install_signals()?;
    let cli = cli::parse(env::args_os().skip(1))?;
    if cli.global.as_deref().is_some_and(|s| s == "--version") {
        println!("game-super-runtime {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if cli
        .global
        .as_deref()
        .is_some_and(|s| s == "--help" || s == "-h")
        || (cli.id.is_none() && cli.global.is_none())
    {
        print!("{}", cli::HELP);
        return Ok(0);
    }
    let paths = Paths::discover(cli.config_dir)?;
    match cli.global.as_deref() {
        Some("--list") => {
            if paths.config.exists() {
                let mut ids = vec![];
                for entry in std::fs::read_dir(&paths.config)? {
                    let path = entry?.path();
                    if path.extension().is_some_and(|s| s == "yaml") {
                        if let Some(id) = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .filter(|s| config::valid_id(s).is_ok())
                        {
                            ids.push(id.to_owned());
                        }
                    }
                }
                ids.sort();
                for id in ids {
                    println!("{id}");
                }
            }
            return Ok(0);
        }
        Some("--edit-defaults" | "--edit-defaults-gui") => {
            let path = paths.ensure_defaults()?;
            if cli.global.as_deref() == Some("--edit-defaults-gui") {
                return game_super_runtime::gui::serve(
                    &path,
                    "Defaults for new games",
                    cli.no_open,
                );
            }
            return editor::edit(&path);
        }
        _ => {}
    }
    let id = cli.id.context("APP-ID is required")?;
    if cli.mode == Mode::ConfigPath {
        println!("{}", paths.app(&id).display());
        return Ok(0);
    }
    if cli.mode == Mode::Logs {
        println!("{}", paths.state.join("logs").join(&id).display());
        return Ok(0);
    }
    if cli.mode == Mode::Launch {
        *log = Some(Log::open(&paths, &id)?);
    }
    let path = paths.ensure_config(&id)?;
    if cli.mode == Mode::Edit {
        return editor::edit(&path);
    }
    if cli.mode == Mode::Gui {
        return game_super_runtime::gui::serve(&path, &id, cli.no_open);
    }
    // Diagnostics remain available even when log storage is unavailable.
    let config = storage::read_config(&path)?;
    if cli.mode == Mode::Validate {
        println!("Valid: {}", path.display());
        return Ok(0);
    }
    let plan = plan::build(&config, &id, &paths.config, &cli.command)?;
    if cli.mode == Mode::DryRun {
        println!("{}", serde_json::to_string_pretty(&plan.json())?);
        return Ok(0);
    }
    if cli.mode == Mode::Doctor {
        execute::preflight(&plan)?;
        execute::command(&plan, &paths.state.join("doctor.log"))?;
        println!(
            "Configuration and host executables OK. Container-only executables are checked when the runtime starts."
        );
        for warning in &plan.warnings {
            println!("Note: {warning}");
        }
        return Ok(0);
    }
    let log = log.as_mut().context("launch log is not available")?;
    let code = execute::run(&plan, &paths, log)?;
    if code != 0 {
        eprintln!(
            "game-super-runtime: launch exited with status {code}. Log: {}",
            log.path.display()
        );
    }
    Ok(code)
}
