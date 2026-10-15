use crate::execute::{executable_exists, exit_code};
use anyhow::{Context, Result, ensure};
use std::{
    env,
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
};

pub fn edit(path: &Path) -> Result<i32> {
    let chosen = env::var("VISUAL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|s| !s.trim().is_empty()));
    let mut argv: Vec<OsString> = if let Some(chosen) = chosen {
        let words = shlex::split(&chosen)
            .context("VISUAL/EDITOR has unmatched quotes; use e.g. EDITOR='code --wait'")?;
        ensure!(!words.is_empty(), "VISUAL/EDITOR is empty");
        words.into_iter().map(Into::into).collect()
    } else {
        let graphical =
            env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some();
        let candidates: &[&str] = if graphical {
            &["xdg-open", "sensible-editor", "editor", "nano", "vi"]
        } else {
            &["sensible-editor", "editor", "nano", "vi"]
        };
        let exe = candidates
            .iter()
            .find(|exe| executable_exists(OsStr::new(exe), None, None))
            .context("no text editor found; set EDITOR to your preferred editor")?;
        vec![OsString::from(exe)]
    };
    argv.push(path.as_os_str().into());
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .status()
        .with_context(|| format!("cannot open editor {}", argv[0].to_string_lossy()))?;
    Ok(exit_code(status))
}

pub fn open_browser(url: &str) -> Result<()> {
    let mut child = Command::new("xdg-open")
        .arg(url)
        .spawn()
        .context("could not open the browser; open the printed URL manually")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
