use game_super_runtime::{config, storage};
use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    net::TcpStream,
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Sandbox {
    root: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gsr-integration-{}", storage::nonce()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_game-super-runtime"));
        c.env("GSR_CONFIG_DIR", self.path("config"))
            .env("XDG_STATE_HOME", self.path("state"))
            .env_remove("STEAM_COMPAT_DATA_PATH")
            .env_remove("STEAM_COMPAT_CLIENT_INSTALL_PATH");
        c
    }
    fn config(&self, text: &str) {
        fs::create_dir_all(self.path("config")).unwrap();
        fs::write(self.path("config/demo.yaml"), text).unwrap();
    }
    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(o: &Output) {
    assert!(
        o.status.success(),
        "status {:?}\nstdout {}\nstderr {}",
        o.status,
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}
fn wait_file(path: &Path, child: &mut Child) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "timeout waiting for {}",
            path.display()
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "child stopped before readiness"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
fn wait_bounded(child: &mut Child) -> i32 {
    let start = Instant::now();
    loop {
        if let Some(s) = child.try_wait().unwrap() {
            return s.code().unwrap_or(-1);
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("launcher did not exit within 10 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
}
fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
fn empty_defaults_preserve_args_output_status_and_inherited_environment() {
    let s = Sandbox::new();
    let o = s.run(&[
        "demo",
        "/bin/bash",
        "-c",
        "printf '%s\\0' \"$@\"; exit 37",
        "arg0",
        "a b",
        "$(false)",
        "",
        "--edit",
    ]);
    assert_eq!(o.status.code(), Some(37));
    assert_eq!(o.stdout, b"a b\0$(false)\0\0--edit\0");
    assert!(s.path("config/demo.yaml").is_file());
    let text = fs::read_to_string(s.path("config/demo.yaml")).unwrap();
    assert_eq!(config::parse(&text).unwrap(), config::Config::default());
}
#[test]
fn non_utf8_steam_arguments_round_trip() {
    let s = Sandbox::new();
    let o = s
        .command()
        .args(["demo", "/usr/bin/printf", "%s"])
        .arg(OsString::from_vec(vec![0xff, b'a']))
        .output()
        .unwrap();
    success(&o);
    assert_eq!(o.stdout, [0xff, b'a']);
}
#[test]
fn non_utf8_arguments_survive_supervision() {
    let s = Sandbox::new();
    s.config("before: [{command: [/bin/true]}]\nshutdown_timeout_ms: 0\n");
    let o = s
        .command()
        .args(["demo", "/usr/bin/printf", "%s"])
        .arg(OsString::from_vec(vec![0xff, b'a']))
        .output()
        .unwrap();
    success(&o);
    assert_eq!(o.stdout, [0xff, b'a']);
}
#[test]
fn defaults_copied_once_and_invalid_defaults_do_not_launch() {
    let s = Sandbox::new();
    fs::create_dir_all(s.path("config")).unwrap();
    fs::write(s.path("config/defaults.yaml"), "env: {GSR_TEST: nope}\n").unwrap();
    let o = s.run(&["demo", "/bin/true"]);
    assert!(!o.status.success());
    assert!(!s.path("config/demo.yaml").exists());
    fs::write(s.path("config/defaults.yaml"), "env: {TEST_GSR: chosen}\n").unwrap();
    let o = s.run(&["demo", "/bin/bash", "-c", "printf %s \"$TEST_GSR\""]);
    success(&o);
    assert_eq!(o.stdout, b"chosen");
    fs::write(s.path("config/defaults.yaml"), "env: {TEST_GSR: changed}\n").unwrap();
    let o = s.run(&["demo", "/bin/bash", "-c", "printf %s \"$TEST_GSR\""]);
    success(&o);
    assert_eq!(o.stdout, b"chosen");
}
#[test]
fn invalid_config_prevents_any_game_execution() {
    let s = Sandbox::new();
    s.config("companions: invalid");
    let o = s.run(&["demo", "/bin/echo", "should not run"]);
    assert_eq!(o.status.code(), Some(125));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains("demo.yaml"));
}
#[test]
fn dry_run_does_not_execute_hooks_or_create_prefix() {
    let s = Sandbox::new();
    let prefix = s.path("prefix");
    s.config(&format!("command: [/bin/true]\nproton: {{path: /fake/proton, prefix: '{}', steam_root: /fake/steam}}\nbefore: [{{command: [touch, '{}']}}]",prefix.display(),s.path("bad").display()));
    let o = s.run(&["demo", "--dry-run"]);
    success(&o);
    assert!(!prefix.exists());
    assert!(!s.path("bad").exists());
    assert!(serde_json::from_slice::<serde_json::Value>(&o.stdout).is_ok());
}
#[test]
fn before_failure_runs_after_and_preserves_status() {
    let s = Sandbox::new();
    s.config("before: [{command: [/bin/bash, -c, 'exit 17']}]\nafter: [{command: [/bin/bash, -c, 'printf after:%s \"$GSR_EXIT_CODE\"']}]\nshutdown_timeout_ms: 0");
    let o = s.run(&["demo", "/bin/echo", "game"]);
    assert_eq!(o.status.code(), Some(17));
    assert_eq!(o.stdout, b"after:17");
}
#[test]
fn hook_timeout_is_bounded_and_allow_failure_continues() {
    let s = Sandbox::new();
    s.config("before: [{command: [/bin/sleep, '30'], timeout_ms: 100, allow_failure: true}]\nshutdown_timeout_ms: 0");
    let t = Instant::now();
    let o = s.run(&["demo", "/bin/echo", "done"]);
    success(&o);
    assert!(t.elapsed() < Duration::from_secs(3));
    assert_eq!(o.stdout, b"done\n");
}
#[test]
fn env_unset_and_script_target_variables() {
    let s = Sandbox::new();
    s.config("variables: {TARGET: 'two words'}\nenv: {GSR_TEST: wrong}");
    assert!(config::parse(&fs::read_to_string(s.path("config/demo.yaml")).unwrap()).is_err());
    s.config("variables: {TARGET: 'two words'}\nunset_env: [REMOVE_ME]\nscripts:\n  prep:\n    body: 'printf \"%s|%s|%s|%s\" \"$GSR_APP_ID\" \"$GSR_VAR_TARGET\" \"$1\" \"${REMOVE_ME-unset}\"'\nbefore: [{script: prep, args: ['a; $(false)']}]\nshutdown_timeout_ms: 0");
    let o = s
        .command()
        .env("REMOVE_ME", "present")
        .args(["demo", "/bin/true"])
        .output()
        .unwrap();
    success(&o);
    assert_eq!(o.stdout, b"demo|two words|a; $(false)|unset");
}
#[test]
fn same_runtime_hosts_game_and_companion_once() {
    let s = Sandbox::new();
    let runtime=s.script("_v2-entry-point",&format!("while [[ $1 != -- ]]; do shift; done; shift\nprintf x >> '{}'\nexport TEST_CONTAINER=inside\nexec \"$@\"",s.path("runtime-count").display()));
    let helper=s.script("helper",&format!("printf '%s' \"$TEST_CONTAINER\" > '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",s.path("ready").display()));
    s.config(&format!("companions:\n  - name: helper\n    command: ['{}']\n    required: true\n    ready_file: '{}'\nbefore:\n  - command: [/bin/bash, -c, 'printf host:%s \'${{TEST_CONTAINER-outside}}\'']\n    scope: host\nshutdown_timeout_ms: 100\n",helper.display(),s.path("ready").display()).replace("'printf host:%s \'${TEST_CONTAINER-outside}\''","'printf host:%s \"${TEST_CONTAINER-outside}\"'"));
    // The before script uses a literal shell expression, escaped from GSR interpolation.
    let mut text = fs::read_to_string(s.path("config/demo.yaml")).unwrap();
    text = text.replace("${TEST_CONTAINER-outside}", "$${TEST_CONTAINER-outside}");
    s.config(&text);
    let o = s
        .command()
        .arg("demo")
        .arg(runtime)
        .args([
            "--verb=waitforexitandrun",
            "--",
            "/bin/bash",
            "-c",
            "printf game:%s \"$TEST_CONTAINER\"",
        ])
        .output()
        .unwrap();
    success(&o);
    assert_eq!(o.stdout, b"host:outsidegame:inside");
    assert_eq!(fs::read(s.path("ready")).unwrap(), b"inside");
    assert_eq!(fs::read(s.path("runtime-count")).unwrap(), b"x");
}
#[test]
fn companion_is_cleaned_up_when_game_exits() {
    let s = Sandbox::new();
    let helper = s.script(
        "helper",
        &format!(
            "echo $$ > '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("pid").display()
        ),
    );
    s.config(&format!("companions: [{{name: helper, command: ['{}'], ready_file: '{}'}}]\nshutdown_timeout_ms: 200",helper.display(),s.path("pid").display()));
    let o = s.run(&["demo", "/bin/true"]);
    success(&o);
    let pid = fs::read_to_string(s.path("pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(!alive(pid));
}
#[test]
fn required_companion_failure_stops_game() {
    let s = Sandbox::new();
    s.config("companions: [{name: doomed, command: [/bin/bash, -c, 'sleep .1; exit 2'], required: true}]\nshutdown_timeout_ms: 0");
    let t = Instant::now();
    let o = s.run(&["demo", "/bin/sleep", "30"]);
    assert_eq!(o.status.code(), Some(125));
    assert!(t.elapsed() < Duration::from_secs(3));
}
#[test]
fn signal_stops_game_companions_and_runs_cleanup() {
    let s = Sandbox::new();
    let game = s.script(
        "game",
        &format!(
            "echo $$ > '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("game-pid").display()
        ),
    );
    let helper = s.script(
        "helper",
        &format!(
            "echo $$ > '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("helper-pid").display()
        ),
    );
    s.config(&format!("companions: [{{name: helper, command: ['{}']}}]\nafter: [{{command: [/usr/bin/touch, '{}']}}]\nshutdown_timeout_ms: 100",helper.display(),s.path("cleanup").display()));
    let mut child = s
        .command()
        .arg("demo")
        .arg(game)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_file(&s.path("game-pid"), &mut child);
    wait_file(&s.path("helper-pid"), &mut child);
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    assert_eq!(wait_bounded(&mut child), 143);
    assert!(s.path("cleanup").exists());
    for name in ["game-pid", "helper-pid"] {
        let pid = fs::read_to_string(s.path(name))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(!alive(pid));
    }
}
#[test]
fn text_editor_handles_spaced_paths_and_creates_defaults() {
    let s = Sandbox::new();
    let editor = s.script(
        "fake editor",
        &format!("printf '%s' \"$1\" > '{}'", s.path("edited").display()),
    );
    let o = s
        .command()
        .env("VISUAL", format!("'{}'", editor.display()))
        .args(["demo", "--edit"])
        .output()
        .unwrap();
    success(&o);
    assert_eq!(
        fs::read_to_string(s.path("edited")).unwrap(),
        s.path("config/demo.yaml").to_string_lossy()
    );
}

#[test]
fn after_game_companion_can_wait_for_game_without_blocking_startup() {
    let s = Sandbox::new();
    let helper = s.script(
        "helper",
        &format!(
            "[[ -f '{}' ]] || exit 9\ntouch '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("game-started").display(),
            s.path("helper-started").display()
        ),
    );
    let game = s.script("game", &format!("touch '{}'\nfor ((i=0;i<40;i++)); do [[ -f '{}' ]] && exit 0; sleep .05; done\nexit 11", s.path("game-started").display(), s.path("helper-started").display()));
    s.config(&format!("companions: [{{name: attach, command: ['{}'], start_after_game: true, start_delay_ms: 100, required: true}}]\nshutdown_timeout_ms: 100", helper.display()));
    let o = s.command().arg("demo").arg(game).output().unwrap();
    success(&o);
    assert!(s.path("helper-started").exists());
}

#[test]
fn after_game_delay_aborts_when_game_is_already_finished() {
    let s = Sandbox::new();
    s.config(&format!("companions: [{{name: late, command: [/usr/bin/touch, '{}'], start_after_game: true, start_delay_ms: 30000}}]\nshutdown_timeout_ms: 0",s.path("late").display()));
    let t = Instant::now();
    let o = s.run(&["demo", "/bin/true"]);
    success(&o);
    assert!(t.elapsed() < Duration::from_secs(3));
    assert!(!s.path("late").exists());
}

#[test]
fn optional_missing_helper_continues_and_required_readiness_times_out() {
    let s = Sandbox::new();
    s.config(
        "companions: [{name: absent, command: [/nonexistent/gsr-helper]}]\nshutdown_timeout_ms: 0",
    );
    success(&s.run(&["demo", "/bin/sleep", ".1"]));
    s.config(&format!("companions: [{{name: never-ready, command: [/bin/sleep, '30'], required: true, ready_file: '{}', ready_timeout_ms: 100}}]\nshutdown_timeout_ms: 0",s.path("missing").display()));
    let o = s.run(&["demo", "/bin/echo", "should-not-run"]);
    assert_eq!(o.status.code(), Some(125));
    assert!(o.stdout.is_empty());
}

#[test]
fn optional_missing_hook_does_not_block_launch() {
    let s = Sandbox::new();
    s.config(
        "before: [{command: [/nonexistent/gsr-hook], allow_failure: true}]\nshutdown_timeout_ms: 0",
    );
    success(&s.run(&["demo", "/bin/true"]));
}

#[test]
fn app_lock_prevents_duplicate_launches_and_releases_on_exit() {
    let s = Sandbox::new();
    let ready = s.path("ready");
    let game = s.script("game", &format!("touch '{}'\nsleep 30", ready.display()));
    let mut first = s
        .command()
        .arg("demo")
        .arg(game)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_file(&ready, &mut first);
    let second = s.run(&["demo", "/bin/true"]);
    assert_eq!(second.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&second.stderr).contains("busy"));
    unsafe {
        libc::kill(first.id() as i32, libc::SIGTERM);
    }
    assert_eq!(wait_bounded(&mut first), 143);
    success(&s.run(&["demo", "/bin/true"]));
}

#[test]
fn custom_proton_override_executes_game_and_helper_in_same_prefix() {
    let s = Sandbox::new();
    let log = s.path("proton.calls");
    let proton=s.script("selected proton",&format!("printf '%s|%s|%s\\n' \"$1\" \"$2\" \"$STEAM_COMPAT_DATA_PATH\" >> '{}'\nshift\nexec \"$@\"",log.display()));
    let helper = s.script(
        "helper",
        &format!(
            "touch '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("ready").display()
        ),
    );
    s.config(&format!("proton: {{path: '{}', prefix: '{}', steam_root: '{}'}}\ncompanions: [{{name: helper, command: ['{}'], proton: true, ready_file: '{}'}}]\nshutdown_timeout_ms: 100",proton.display(),s.path("prefix").display(),s.root.display(),helper.display(),s.path("ready").display()));
    let o = s.run(&[
        "demo",
        "/fake/original/proton",
        "waitforexitandrun",
        "/bin/true",
    ]);
    success(&o);
    let calls = fs::read_to_string(log).unwrap();
    assert_eq!(calls.lines().count(), 2);
    assert!(calls.lines().all(|line| line.starts_with("run|")));
    assert!(
        calls
            .lines()
            .all(|line| line.ends_with(s.path("prefix").to_str().unwrap()))
    );
}

#[test]
fn nested_runtime_gets_time_for_inner_after_hooks_on_signal() {
    let s = Sandbox::new();
    let runtime = s.script("_v2-entry-point", "shift; exec \"$@\"");
    let game = s.script(
        "game",
        &format!(
            "touch '{}'\ntrap 'exit 0' TERM\nwhile :; do sleep .1; done",
            s.path("ready").display()
        ),
    );
    s.config(&format!("before: [{{command: [/bin/true], scope: host}}]\nafter: [{{command: [/bin/bash, -c, 'sleep .3; touch \"{}\"'], scope: game}}]\nshutdown_timeout_ms: 50",s.path("cleanup").display()));
    let mut child = s
        .command()
        .arg("demo")
        .arg(runtime)
        .arg("--")
        .arg(game)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_file(&s.path("ready"), &mut child);
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    assert_eq!(wait_bounded(&mut child), 143);
    assert!(s.path("cleanup").exists());
}

fn http(authority: &str, path: &str, method: &str, token: &str, body: &str, extra: &str) -> String {
    let mut s = TcpStream::connect(authority).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s,"{method} {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nX-GSR-Token: {token}\r\nContent-Length: {}\r\n{extra}\r\n{body}",body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}
#[test]
fn gui_api_validates_saves_and_rejects_cross_origin() {
    let s = Sandbox::new();
    let mut child = s
        .command()
        .args(["demo", "--edit-gui", "--no-open"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::BufRead;
    let mut line = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line.trim().split("http://").nth(1).unwrap();
    let (authority, tail) = url.split_once('/').unwrap();
    let base = format!("/{tail}");
    let token = tail.trim_end_matches('/');
    let r = http(authority, &format!("{base}config"), "GET", token, "", "");
    assert!(r.starts_with("HTTP/1.1 200"));
    let data: serde_json::Value =
        serde_json::from_str(r.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let r = http(
        authority,
        &format!("{base}save"),
        "POST",
        token,
        &serde_json::json!({"yaml":"bad: true","original":data["yaml"]}).to_string(),
        "",
    );
    assert!(r.starts_with("HTTP/1.1 400"));
    let r = http(
        authority,
        &format!("{base}save"),
        "POST",
        token,
        &serde_json::json!({"yaml":"version: 1\nname: changed\n","original":data["yaml"]})
            .to_string(),
        "Origin: https://untrusted.invalid\r\n",
    );
    assert!(r.starts_with("HTTP/1.1 403"));
    let r = http(
        authority,
        &format!("{base}save"),
        "POST",
        token,
        &serde_json::json!({"yaml":"version: 1\nname: changed\n","original":data["yaml"]})
            .to_string(),
        "",
    );
    assert!(r.starts_with("HTTP/1.1 200"));
    assert!(s.path("config/demo.yaml.bak").exists());
    http(authority, &format!("{base}close"), "POST", token, "{}", "");
    assert_eq!(wait_bounded(&mut child), 0);
}
