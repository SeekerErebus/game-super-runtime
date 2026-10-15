use game_super_runtime::{config, plan, storage};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};
struct Sandbox {
    root: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("gsr-virtual-{}", storage::nonce()));
        for name in [
            "config", "base", "first", "last", "app", "target", "appdata",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        Self { root }
    }
    fn config(&self, extra: &str) {
        fs::write(
            self.root.join("config/demo.yaml"),
            format!("version: 1\nshutdown_timeout_ms: 100\n{extra}"),
        )
        .unwrap();
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_game-super-runtime"));
        c.env("GSR_CONFIG_DIR", self.root.join("config"))
            .env("XDG_STATE_HOME", self.root.join("state"));
        c
    }
    fn overlay(&self) -> String {
        format!(
            "virtual_directories:\n  - type: overlay\n    target: {0}/base\n    write_dir: {0}/writes\n    manifest: {0}/mods.yaml\n",
            self.root.display()
        )
    }
    fn manifest(&self) {
        fs::write(self.root.join("mods.yaml"), "version: 1\nlayers:\n  - path: first\n  - path: missing-disabled\n    enabled: false\n  - path: last\n").unwrap();
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(o: Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn config_rejects_ambiguous_or_unknown_mapping_fields() {
    for fields in [
        "type: symlink\n    source: a\n    target: b",
        "type: bind\n    source: a\n    target: ''",
        "type: overlay\n    target: b\n    write_dir: c\n    layers: [{path: d}]\n    manifest: m",
        "type: bind\n    source: a\n    target: b\n    typo: yes",
    ] {
        assert!(
            config::parse(&format!("version: 1\nvirtual_directories:\n  - {fields}\n")).is_err()
        );
    }
}
#[test]
fn manifest_is_versioned_ordered_relative_and_strict() {
    let s = Sandbox::new();
    s.manifest();
    s.config(&s.overlay());
    let c = storage::read_config(&s.root.join("config/demo.yaml")).unwrap();
    let p = plan::build(&c, "demo", &s.root.join("config"), &["/bin/true".into()]).unwrap();
    assert_eq!(
        p.virtual_directories[0].layers,
        [s.root.join("first"), s.root.join("last")]
    );
    for bad in [
        "version: 2\nlayers: []",
        "version: 1\nlayers: []\nunknown: true",
        "version: 1\nlayers: [{path: first, path: last}]",
    ] {
        fs::write(s.root.join("mods.yaml"), bad).unwrap();
        assert!(plan::build(&c, "demo", &s.root.join("config"), &["/bin/true".into()]).is_err());
    }
}
#[test]
fn dependency_is_optional_and_missing_tool_error_precedes_launch() {
    let s = Sandbox::new();
    s.config("");
    success(
        s.command()
            .env("PATH", "")
            .args(["demo", "/bin/true"])
            .output()
            .unwrap(),
    );
    s.config(&format!(
        "virtual_directories:\n  - type: bind\n    source: {0}/app\n    target: {0}/target\n",
        s.root.display()
    ));
    let o = s
        .command()
        .env("PATH", "")
        .args(["demo", "/bin/true"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&o.stderr).contains("requires bwrap"));
    assert!(
        !s.root
            .join("state/game-super-runtime/virtual-sessions")
            .exists()
    );
}

#[test]
#[ignore = "requires working bubblewrap, fuse-overlayfs, fuse3 and /dev/fuse; run explicitly"]
fn real_overlay_precedence_copy_up_deletion_persistence_and_cleanup() {
    let s = Sandbox::new();
    s.manifest();
    s.config(&s.overlay());
    fs::write(s.root.join("base/shared"), "base").unwrap();
    fs::write(s.root.join("base/deleted"), "original").unwrap();
    fs::create_dir(s.root.join("first/nested")).unwrap();
    fs::write(s.root.join("first/nested/first"), "first").unwrap();
    fs::write(s.root.join("first/shared"), "first").unwrap();
    fs::write(s.root.join("last/shared"), "last").unwrap();
    let script = format!(
        "set -eu; cd '{}'; test \"$(cat shared)\" = last; test \"$(cat nested/first)\" = first; echo changed > shared; echo new > created; rm deleted",
        s.root.join("base").display()
    );
    success(
        s.command()
            .args(["demo", "/bin/bash", "-c", &script])
            .output()
            .unwrap(),
    );
    assert_eq!(
        fs::read_to_string(s.root.join("base/shared")).unwrap(),
        "base"
    );
    assert!(s.root.join("base/deleted").exists());
    assert!(!s.root.join("base/created").exists());
    assert_eq!(
        fs::read_to_string(s.root.join("last/shared")).unwrap(),
        "last"
    );
    let script = format!(
        "set -eu; cd '{}'; test \"$(cat shared)\" = changed; test \"$(cat created)\" = new; test ! -e deleted",
        s.root.join("base").display()
    );
    success(
        s.command()
            .args(["demo", "/bin/bash", "-c", &script])
            .output()
            .unwrap(),
    );
    let sessions = s.root.join("state/game-super-runtime/virtual-sessions");
    assert_eq!(fs::read_dir(sessions).unwrap().count(), 0);
}

#[test]
#[ignore = "requires working bubblewrap"]
fn real_bind_is_private_and_read_only_or_shared_writable() {
    let s = Sandbox::new();
    fs::write(s.root.join("app/exe"), "application").unwrap();
    fs::write(s.root.join("target/original"), "untouched").unwrap();
    s.config(&format!("virtual_directories:\n  - type: bind\n    source: {0}/app\n    target: {0}/target\n  - type: bind\n    source: {0}/appdata\n    target: {0}/base\n    read_only: false\n", s.root.display()));
    let script = format!(
        "set -eu; test \"$(cat '{0}/target/exe')\" = application; test ! -e '{0}/target/original'; if touch '{0}/target/forbidden' 2>/dev/null; then exit 1; fi; echo updated > '{0}/base/settings'",
        s.root.display()
    );
    success(
        s.command()
            .args(["demo", "/bin/bash", "-c", &script])
            .output()
            .unwrap(),
    );
    assert!(s.root.join("target/original").exists());
    assert!(!s.root.join("target/exe").exists());
    assert_eq!(
        fs::read_to_string(s.root.join("appdata/settings")).unwrap(),
        "updated\n"
    );
}

#[test]
#[ignore = "requires working bubblewrap, fuse-overlayfs, fuse3 and /dev/fuse"]
fn real_overlay_signal_cleanup_and_write_lock() {
    let s = Sandbox::new();
    s.manifest();
    s.config(&s.overlay());
    let ready = s.root.join("ready");
    let mut child = s
        .command()
        .args([
            "demo",
            "/bin/bash",
            "-c",
            &format!("touch '{}'; exec sleep 30", ready.display()),
        ])
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before ready"
        );
        assert!(start.elapsed() < Duration::from_secs(15));
        thread::sleep(Duration::from_millis(20));
    }
    fs::copy(
        s.root.join("config/demo.yaml"),
        s.root.join("config/other.yaml"),
    )
    .unwrap();
    let o = s.command().args(["other", "/bin/true"]).output().unwrap();
    assert_eq!(o.status.code(), Some(125));
    assert!(String::from_utf8_lossy(&o.stderr).contains("busy"));
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    assert_eq!(child.wait().unwrap().code(), Some(143));
    assert_eq!(
        fs::read_dir(s.root.join("state/game-super-runtime/virtual-sessions"))
            .unwrap()
            .count(),
        0
    );
    success(s.command().args(["other", "/bin/true"]).output().unwrap());
}

#[test]
#[ignore = "requires working bubblewrap, fuse-overlayfs, fuse3 and /dev/fuse"]
fn real_partial_setup_failure_rolls_back_prior_mount() {
    let s = Sandbox::new();
    s.manifest();
    fs::create_dir(s.root.join("bad-writes")).unwrap();
    fs::write(s.root.join("bad-writes/upper"), "do not replace").unwrap();
    s.config(&format!(
        "{}  - type: overlay\n    target: {}/target\n    write_dir: {}/bad-writes\n",
        s.overlay(),
        s.root.display(),
        s.root.display()
    ));
    let o = s.command().args(["demo", "/bin/true"]).output().unwrap();
    assert_eq!(o.status.code(), Some(125));
    assert_eq!(
        fs::read_to_string(s.root.join("bad-writes/upper")).unwrap(),
        "do not replace"
    );
    assert_eq!(
        fs::read_dir(s.root.join("state/game-super-runtime/virtual-sessions"))
            .unwrap()
            .count(),
        0
    );
    s.config(&s.overlay());
    success(s.command().args(["demo", "/bin/true"]).output().unwrap());
}

#[test]
#[ignore = "requires working bubblewrap"]
fn executable_and_cwd_can_exist_only_in_the_virtual_view() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    fs::create_dir(s.root.join("app/bin")).unwrap();
    fs::write(
        s.root.join("app/bin/run"),
        "#!/bin/sh\ntest \"$PWD\" = \"$1\"\n",
    )
    .unwrap();
    fs::set_permissions(
        s.root.join("app/bin/run"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    s.config(&format!("working_dir: '{0}/target/bin'\ncommand: ['./run', '{0}/target/bin']\nvirtual_directories:\n  - type: bind\n    source: '{0}/app'\n    target: '{0}/target'\n", s.root.display()));
    success(s.command().arg("demo").output().unwrap());
    assert!(!s.root.join("target/bin").exists());
}

#[test]
#[ignore = "requires installed mount tools and /dev/fuse for preflight"]
fn unsafe_layouts_are_rejected_without_launch_or_target_creation() {
    use std::os::unix::fs::symlink;
    let s = Sandbox::new();
    s.manifest();
    symlink(s.root.join("base"), s.root.join("alias")).unwrap();
    for mapping in [
        format!(
            "  - type: bind\n    source: {0}/app\n    target: {0}/absent",
            s.root.display()
        ),
        format!(
            "  - type: bind\n    source: {0}/app\n    target: {0}/base\n  - type: bind\n    source: {0}/first\n    target: {0}/alias",
            s.root.display()
        ),
        format!(
            "  - type: overlay\n    target: {0}/base\n    write_dir: {0}/alias/writes",
            s.root.display()
        ),
        format!(
            "  - type: bind\n    source: {0}/alias\n    target: {0}/base",
            s.root.display()
        ),
    ] {
        s.config(&format!("virtual_directories:\n{mapping}\n"));
        let o = s
            .command()
            .args(["demo", "/usr/bin/touch"])
            .arg(s.root.join("launched"))
            .output()
            .unwrap();
        assert_eq!(
            o.status.code(),
            Some(125),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    assert!(!s.root.join("launched").exists());
    assert!(!s.root.join("absent").exists());
    assert!(!s.root.join("base/writes").exists());
}

#[test]
#[ignore = "requires working bubblewrap, fuse-overlayfs, fuse3 and /dev/fuse"]
fn fuse_service_failure_stops_launch_and_releases_mount_storage() {
    let s = Sandbox::new();
    s.manifest();
    s.config(&s.overlay());
    let ready = s.root.join("ready");
    let mut child = s
        .command()
        .args([
            "demo",
            "/bin/bash",
            "-c",
            &format!("touch '{}'; exec sleep 30", ready.display()),
        ])
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() {
        assert!(child.try_wait().unwrap().is_none());
        assert!(start.elapsed() < Duration::from_secs(15));
        thread::sleep(Duration::from_millis(20));
    }
    let children = fs::read_to_string(format!("/proc/{0}/task/{0}/children", child.id())).unwrap();
    let service = children
        .split_whitespace()
        .find(|pid| {
            fs::read(format!("/proc/{pid}/cmdline"))
                .is_ok_and(|cmd| cmd.starts_with(b"fuse-overlayfs\0"))
        })
        .expect("foreground FUSE service")
        .parse::<i32>()
        .unwrap();
    unsafe {
        libc::kill(service, libc::SIGKILL);
    }
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert_eq!(status.code(), Some(125));
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            panic!("launch survived FUSE failure");
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        fs::read_dir(s.root.join("state/game-super-runtime/virtual-sessions"))
            .unwrap()
            .count(),
        0
    );
    success(s.command().args(["demo", "/bin/true"]).output().unwrap());
}
