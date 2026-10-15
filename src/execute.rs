use crate::{
    config::Scope,
    plan::{self, Plan, Task},
    storage::{self, FileLock, Paths},
};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::{AtomicI32, Ordering},
    thread,
    time::{Duration, Instant},
};

static SIGNAL: AtomicI32 = AtomicI32::new(0);
extern "C" fn handle_signal(sig: libc::c_int) {
    SIGNAL.store(sig, Ordering::Relaxed);
}
pub fn install_signals() -> Result<()> {
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: zeroed sigaction is initialized below; the handler only stores an atomic.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = handle_signal as *const () as usize;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
        }
        action.sa_flags = 0;
        ensure!(
            unsafe { libc::sigaction(sig, &action, std::ptr::null_mut()) } == 0,
            "cannot install signal handler: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}
pub fn received_signal() -> i32 {
    SIGNAL.load(Ordering::Relaxed)
}
pub fn exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
}

pub struct Log {
    pub path: PathBuf,
    file: File,
}
impl Log {
    pub fn open(paths: &Paths, app_id: &str) -> Result<Self> {
        let dir = paths.state.join("logs").join(app_id);
        storage::private_dir(&dir)?;
        let path = dir.join(format!("{}.log", storage::nonce()));
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut old: Vec<_> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().is_some_and(|x| x == "log")
                    && e.file_type().is_ok_and(|t| t.is_file())
            })
            .collect();
        old.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
        for entry in old.iter().take(old.len().saturating_sub(10)) {
            let _ = fs::remove_file(entry.path());
        }
        Ok(Self { path, file })
    }
    pub fn write(&mut self, text: &str) {
        let _ = writeln!(self.file, "{text}");
    }
}

pub fn quote(word: &OsStr) -> String {
    if let Some(s) = word.to_str() {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        format!(
            "$'{}'",
            word.as_bytes()
                .iter()
                .map(|b| format!("\\x{b:02x}"))
                .collect::<String>()
        )
    }
}
pub fn quote_argv(argv: &[OsString]) -> String {
    argv.iter().map(|s| quote(s)).collect::<Vec<_>>().join(" ")
}

fn function(name: &str, task: &Task) -> String {
    let mut out = format!("{name}() {{\n");
    // Each function runs as a background subshell; these changes never leak.
    if let Some(cwd) = &task.cwd {
        out.push_str(&format!("cd -- {} || return 125\n", quote(cwd.as_os_str())));
    }
    for (k, v) in &task.env {
        out.push_str(&format!("export {k}={}\n", quote(OsStr::new(v))));
    }
    out.push_str(&format!("exec {}\n}}\n", quote_argv(&task.argv)));
    out
}
fn script(
    plan: &Plan,
    scope: Scope,
    game: Vec<OsString>,
    log: &Path,
    collapse_scopes: bool,
) -> Result<Vec<OsString>> {
    let in_scope = |s| collapse_scopes || s == scope;
    let before: Vec<_> = plan.before.iter().filter(|t| in_scope(t.scope)).collect();
    let after: Vec<_> = plan.after.iter().filter(|t| in_scope(t.scope)).collect();
    let companions: Vec<_> = plan
        .companions
        .iter()
        .filter(|c| in_scope(c.task.scope))
        .collect();
    if before.is_empty() && after.is_empty() && companions.is_empty() {
        return Ok(game);
    }
    let main_grace_ms = if scope == Scope::Host && !plan.runtime.is_empty() {
        plan.shutdown_timeout_ms * 2
            + plan
                .after
                .iter()
                .filter(|t| t.scope == Scope::Game)
                .map(|t| t.timeout_ms.unwrap_or(30000) + 100)
                .sum::<u64>()
            + 500
    } else {
        plan.shutdown_timeout_ms
    };
    let mut code = format!(
        "gsr_log_file={}\ngsr_shutdown_ticks={}\ngsr_main_grace_ticks={}\n",
        quote(log.as_os_str()),
        plan.shutdown_timeout_ms.div_ceil(50),
        main_grace_ms.div_ceil(50)
    );
    code.push_str(include_str!("../assets/supervisor.bash"));
    let main = Task {
        name: "game".into(),
        argv: game,
        env: Default::default(),
        cwd: None,
        scope,
        allow_failure: false,
        timeout_ms: None,
    };
    code.push_str(&function("gsr_game", &main));
    for (prefix, tasks) in [("before", &before), ("after", &after)] {
        for (i, task) in tasks.iter().enumerate() {
            code.push_str(&function(&format!("gsr_{prefix}_{i}"), task));
        }
    }
    for (i, companion) in companions.iter().enumerate() {
        code.push_str(&function(&format!("gsr_companion_{i}"), &companion.task));
    }
    code.push_str("gsr_after() {\nlocal result=0 rc\n");
    for (i, task) in after.iter().enumerate() {
        code.push_str(&format!("gsr_hook {} {} {} gsr_after_{i}; rc=$?\nif [[ $result == 0 && $rc != 0 ]]; then result=$rc; fi\n",
            quote(OsStr::new(&task.name)), task.timeout_ms.unwrap_or(30000), u8::from(task.allow_failure)));
    }
    code.push_str("return \"$result\"\n}\n");
    for (i, task) in before.iter().enumerate() {
        code.push_str(&format!(
            "gsr_hook {} {} {} gsr_before_{i} || exit $?\n",
            quote(OsStr::new(&task.name)),
            task.timeout_ms.unwrap_or(30000),
            u8::from(task.allow_failure)
        ));
    }
    for after_game in [false, true] {
        if after_game {
            code.push_str(
                "gsr_check_companions || exit $?\ngsr_start gsr_game\ngsr_main=$gsr_pid\n",
            );
        }
        for (i, companion) in companions
            .iter()
            .enumerate()
            .filter(|(_, c)| c.start_after_game == after_game)
        {
            code.push_str(&format!(
                "gsr_companion {} {} {} {} {} gsr_companion_{i} || exit $?\n",
                quote(OsStr::new(&companion.task.name)),
                u8::from(companion.required),
                companion.start_delay_ms,
                quote(
                    companion
                        .ready_file
                        .as_deref()
                        .unwrap_or(Path::new(""))
                        .as_os_str()
                ),
                companion.ready_timeout_ms
            ));
        }
    }
    code.push_str("while gsr_alive \"$gsr_main\"; do\ngsr_prune_groups\ngsr_check_companions || exit $?\nsleep 0.05\ndone\nwait \"$gsr_main\"\nexit $?\n");
    ensure!(
        code.len() < 120000,
        "generated supervisor exceeds Linux's argument size limit; move large script bodies to scripts.NAME.path"
    );
    Ok(vec![
        "/bin/bash".into(),
        "--noprofile".into(),
        "--norc".into(),
        "-c".into(),
        code.into(),
        "gsr-supervisor".into(),
    ])
}

pub fn command(plan: &Plan, log: &Path) -> Result<Vec<OsString>> {
    let mut argv = if plan.runtime.is_empty() {
        script(plan, Scope::Game, plan.game.clone(), log, true)?
    } else {
        let inner = script(plan, Scope::Game, plan.game.clone(), log, false)?;
        let runtime = plan::wrap(&plan.runtime, inner);
        script(plan, Scope::Host, runtime, log, false)?
    };
    for wrapper in plan.wrappers.iter().rev() {
        argv = plan::wrap(wrapper, argv);
    }
    Ok(argv)
}

pub fn executable_exists(executable: &OsStr, cwd: Option<&Path>, path: Option<&OsStr>) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let check = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if executable.as_bytes().contains(&b'/') {
        let p = Path::new(executable);
        return check(&if p.is_absolute() {
            p.to_owned()
        } else {
            cwd.unwrap_or(Path::new(".")).join(p)
        });
    }
    let search = path
        .map(OsStr::to_owned)
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_else(|| "/usr/bin:/bin".into());
    std::env::split_paths(&search).any(|p| {
        check(&if p.is_absolute() {
            p.join(executable)
        } else {
            cwd.unwrap_or(Path::new(".")).join(p).join(executable)
        })
    })
}

pub fn preflight(plan: &Plan) -> Result<()> {
    if let Some(cwd) = &plan.cwd {
        ensure!(
            cwd.is_dir(),
            "working_dir does not exist: {}",
            cwd.display()
        );
    }
    let path = plan.env.get("PATH").map(OsStr::new);
    let check = |exe: &OsStr, cwd: Option<&Path>, label: &str| -> Result<()> {
        ensure!(
            executable_exists(exe, cwd.or(plan.cwd.as_deref()), path),
            "{label}: executable {} is missing or not executable; install it or correct the configuration",
            exe.to_string_lossy()
        );
        Ok(())
    };
    for wrapper in &plan.wrappers {
        check(&wrapper[0], None, "wrapper")?;
    }
    if plan.runtime.is_empty() {
        check(&plan.game[0], None, "game")?;
    } else {
        check(&plan.runtime[0], None, "runtime")?;
    }
    if plan.supervised() {
        check(OsStr::new("/bin/bash"), None, "supervisor")?;
    }
    for task in plan
        .before
        .iter()
        .chain(&plan.after)
        .chain(
            plan.companions
                .iter()
                .filter(|c| c.required)
                .map(|c| &c.task),
        )
        .filter(|task| !task.allow_failure)
    {
        if plan.runtime.is_empty() || task.scope == Scope::Host {
            let task_path = task.env.get("PATH").map(OsStr::new).or(path);
            ensure!(
                executable_exists(
                    &task.argv[0],
                    task.cwd.as_deref().or(plan.cwd.as_deref()),
                    task_path
                ),
                "{}: executable {} is missing or not executable",
                task.name,
                task.argv[0].to_string_lossy()
            );
        }
    }
    if let Some(proton) = &plan.proton {
        if proton.is_absolute() {
            check(proton.as_os_str(), None, "Proton")?;
        }
    }
    Ok(())
}

pub fn run(plan: &Plan, paths: &Paths, log: &mut Log) -> Result<i32> {
    preflight(plan)?;
    let lock_dir = paths.state.join("locks");
    storage::private_dir(&lock_dir)?;
    let _lock = FileLock::acquire(&lock_dir.join(format!("{}.lock", plan.app_id)))?;
    if let Some(prefix) = &plan.prefix {
        if plan.env.contains_key("STEAM_COMPAT_DATA_PATH") {
            storage::private_dir(prefix)?;
        }
    }
    let argv = command(plan, &log.path)?;
    log.write(&format!(
        "Starting app {} ({} companion(s), shared runtime: {}).",
        plan.app_id,
        plan.companions.len(),
        !plan.runtime.is_empty()
    ));
    for warning in &plan.warnings {
        eprintln!("game-super-runtime: {warning}");
        log.write(warning);
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).envs(&plan.env).process_group(0);
    for key in &plan.unset_env {
        cmd.env_remove(key);
    }
    if let Some(cwd) = &plan.cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("cannot launch {}", argv[0].to_string_lossy()))?;
    let pid = child.id() as i32;
    let mut signal_sent = None;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break exit_code(status),
            Ok(None) => {}
            Err(e) => {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
                let _ = child.wait();
                return Err(e).context("cannot wait for game process");
            }
        }
        let sig = received_signal();
        if sig != 0 && signal_sent.is_none() {
            log.write(&format!(
                "Received signal {sig}; forwarding to launch process group."
            ));
            // SAFETY: negative PID addresses only the group created for our child.
            unsafe {
                libc::kill(-pid, sig);
            }
            signal_sent = Some(Instant::now());
        }
        // Leave time for two supervisor layers plus bounded after hooks to clean up.
        let grace = if plan.supervised() {
            plan.shutdown_timeout_ms * 4
                + plan
                    .after
                    .iter()
                    .map(|t| t.timeout_ms.unwrap_or(30000) + 100)
                    .sum::<u64>()
                + 2000
        } else {
            plan.shutdown_timeout_ms
        };
        if signal_sent.is_some_and(|t| t.elapsed() > Duration::from_millis(grace)) {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        thread::sleep(Duration::from_millis(20));
    };
    let code = if received_signal() != 0 {
        128 + received_signal()
    } else {
        code
    };
    log.write(&format!("App {} exited with status {code}.", plan.app_id));
    Ok(code)
}
