use game_super_runtime::{
    cli::{self, Mode},
    config::{self, Config},
    execute, plan, storage,
};
use std::{ffi::OsString, path::Path};

fn strings(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}
fn build(yaml: &str, command: &[&str]) -> anyhow::Result<plan::Plan> {
    plan::build(
        &config::parse(yaml)?,
        "1091500",
        Path::new("/tmp/gsr-test-config"),
        &strings(command),
    )
}

#[test]
fn empty_configuration_is_literal_passthrough() {
    let args = strings(&[
        "/bin/echo",
        "a b",
        "'quotes'",
        "$(touch /tmp/no)",
        "${HOME}",
        "",
        "--edit",
    ]);
    let p = plan::build(&Config::default(), "demo", Path::new("/tmp"), &args).unwrap();
    assert_eq!(execute::command(&p, Path::new("/tmp/log")).unwrap(), args);
    assert!(p.env.is_empty());
    assert!(p.unset_env.is_empty());
    assert!(p.cwd.is_none());
}

#[test]
fn opaque_container_command_is_untouched_without_advanced_manipulation() {
    let args = strings(&["/runtime/_v2-entry-point", "/game"]);
    let p = plan::build(&Config::default(), "demo", Path::new("/tmp"), &args).unwrap();
    assert_eq!(execute::command(&p, Path::new("/tmp/log")).unwrap(), args);
    assert!(
        build(
            "runtime: {mode: required}",
            &["/runtime/_v2-entry-point", "/game"]
        )
        .is_err()
    );
}
#[test]
fn steam_chain_round_trips_without_changes() {
    let args = strings(&[
        "/steam/reaper",
        "SteamLaunch",
        "AppId=1",
        "--",
        "/steam/steam-launch-wrapper",
        "--",
        "/steam/SteamLinuxRuntime_sniper/_v2-entry-point",
        "--verb=waitforexitandrun",
        "--",
        "/steam/Proton/proton",
        "waitforexitandrun",
        "a game.exe",
        "--edit",
    ]);
    let p = plan::build(&Config::default(), "1", Path::new("/tmp"), &args).unwrap();
    assert_eq!(execute::command(&p, Path::new("/tmp/log")).unwrap(), args);
}
#[test]
fn invalid_yaml_is_actionable() {
    for yaml in [
        "version: 2",
        "frobnicate: true",
        "env: {A: 1, A: 2}",
        "overlays: {typo: true}",
        "env: {BAD-NAME: x}",
        "companions: [{name: helper}]",
        "command: echo hello",
        "[1, 2]",
        "proton: {verb: destroyprefix}",
        "scripts: {a: {body: hi, path: bye}}",
        "before: [{script: missing}]",
        "shutdown_timeout_ms: 900000",
        "runtime: {mode: none, command: [x]}",
        "command: ['']",
        "command: [\"a\\0b\"]",
        "env: {GSR_PROTON: x}",
    ] {
        assert!(config::parse(yaml).is_err(), "should reject {yaml:?}");
    }
    assert_eq!(config::parse("").unwrap(), Config::default());
    assert_eq!(config::parse("# comments only").unwrap(), Config::default());
}
#[test]
fn app_ids_cannot_escape_storage() {
    for id in [
        "", ".", "..", "../x", "a/b", "x\n", "defaults", ".hidden", "a b",
    ] {
        assert!(config::valid_id(id).is_err(), "{id}");
    }
    for id in ["1091500", "my-game_2.0", "Demo"] {
        config::valid_id(id).unwrap();
    }
}
#[test]
fn expansion_is_explicit_recursive_and_not_a_shell() {
    let p=build("variables:\n  ROOT: '${CONFIG_DIR}/games'\n  GAME: '${vars.ROOT}/demo'\ncommand: [echo, '${vars.GAME}', '$$(touch bad)', '$${HOME}']\nenv: {A: '${APP_ID}'}",&[]).unwrap();
    assert_eq!(
        p.game,
        strings(&[
            "echo",
            "/tmp/gsr-test-config/games/demo",
            "$(touch bad)",
            "${HOME}"
        ])
    );
    assert_eq!(p.env["A"], "1091500");
    assert!(
        build(
            "variables: {A: '${vars.B}', B: '${vars.A}'}\ncommand: [echo, '${vars.A}']",
            &[]
        )
        .is_err()
    );
    assert!(build("command: [echo, '${missing}']", &[]).is_err());
}
#[test]
fn wrappers_have_predictable_order() {
    let p=build("wrappers: [[outer, x], [middle]]\ngame_wrappers: [[inner]]\noverlays: {gamescope: ['-f'], gamemode: true, mangohud: true, vkbasalt: true}",&["game", "arg"]).unwrap();
    assert_eq!(
        execute::command(&p, Path::new("/tmp/log")).unwrap(),
        strings(&[
            "outer",
            "x",
            "middle",
            "gamescope",
            "-f",
            "--",
            "gamemoderun",
            "inner",
            "game",
            "arg"
        ])
    );
    assert_eq!(p.env["MANGOHUD"], "1");
    assert_eq!(p.env["ENABLE_VKBASALT"], "1");
}
#[test]
fn game_arguments_cannot_spoof_container_boundaries() {
    let args = strings(&[
        "echo",
        "/tmp/_v2-entry-point",
        "--",
        "proton",
        "run",
        "file",
    ]);
    assert_eq!(plan::runtime_boundary(&args).unwrap(), None);
    let p = build(
        "runtime: {mode: required}",
        &["echo", "_v2-entry-point", "--", "game"],
    );
    assert!(p.is_err());
}
#[test]
fn proton_companions_use_same_proton_and_run_verb() {
    let yaml = "proton: {prefix: /tmp/compat, steam_root: /tmp/steam}\ncompanions:\n  - name: helper\n    command: [helper.exe]\n    proton: true\n";
    let p = build(
        yaml,
        &[
            "/tmp/_v2-entry-point",
            "--",
            "/tmp/Proton/proton",
            "waitforexitandrun",
            "game.exe",
        ],
    )
    .unwrap();
    assert_eq!(p.game, strings(&["/tmp/Proton/proton", "run", "game.exe"]));
    assert_eq!(
        p.companions[0].task.argv,
        strings(&["/tmp/Proton/proton", "run", "helper.exe"])
    );
    assert_eq!(p.env["STEAM_COMPAT_DATA_PATH"], "/tmp/compat");
    assert!(
        build(
            &yaml.replace("prefix:", "verb: waitforexitandrun, prefix:"),
            &["/tmp/proton", "waitforexitandrun", "game"]
        )
        .is_err()
    );
}
#[test]
fn explicit_runtime_and_proton_are_composable() {
    let p=build("command: [game.exe]\nproton: {path: /tmp/ge/proton, prefix: /tmp/prefix, steam_root: /tmp/steam}\nruntime: {mode: required, command: [/tmp/runtime/_v2-entry-point, --verb=run, --]}",&[]).unwrap();
    assert_eq!(
        p.game,
        strings(&["/tmp/ge/proton", "waitforexitandrun", "game.exe"])
    );
    assert_eq!(p.runtime.len(), 3);
    assert!(build("proton: {prefix: /tmp/game/pfx}", &["echo"]).is_err());
}
#[test]
fn scripts_receive_arguments_without_expanding_bash_body() {
    let p=build("scripts: {prep: {body: 'echo \"${GSR_APP_ID}\" \"$1\"'}}\nbefore: [{script: prep, args: ['two words']}]",&["true"]).unwrap();
    assert!(
        p.before[0]
            .argv
            .contains(&OsString::from("echo \"${GSR_APP_ID}\" \"$1\""))
    );
    assert_eq!(p.before[0].argv.last().unwrap(), "two words");
}

#[test]
fn scripts_allow_an_empty_first_argument() {
    let p = build(
        "scripts: {s: {body: 'printf %s \"$1\"'}}\nbefore: [{script: s, args: ['', second]}]",
        &["true"],
    )
    .unwrap();
    assert_eq!(
        &p.before[0].argv[p.before[0].argv.len() - 2..],
        strings(&["", "second"])
    );
}
#[test]
fn cli_stops_parsing_at_game_command() {
    let c = cli::parse(strings(&["42", "--dry-run", "/game", "--edit", "--help"])).unwrap();
    assert_eq!(c.mode, Mode::DryRun);
    assert_eq!(c.command, strings(&["/game", "--edit", "--help"]));
    assert!(cli::parse(strings(&["42", "--unknown"])).is_err());
    assert!(cli::parse(strings(&["42", "--edit", "--validate"])).is_err());
}
#[test]
fn config_limit_and_duplicate_nested_maps() {
    assert!(config::parse(&"x".repeat(config::MAX_CONFIG_BYTES + 1)).is_err());
    assert!(config::parse("variables: {A: one, A: two}").is_err());
}

#[test]
fn replacement_proton_keeps_its_position_behind_reaper() {
    let p = build("proton: {path: /tmp/custom-proton, prefix: /tmp/compat, steam_root: /tmp/steam}\ncompanions: [{name: helper, command: [helper.exe], proton: true}]", &["/tmp/reaper", "SteamLaunch", "--", "/tmp/proton", "waitforexitandrun", "game.exe"]).unwrap();
    assert_eq!(
        p.game,
        strings(&[
            "/tmp/reaper",
            "SteamLaunch",
            "--",
            "/tmp/custom-proton",
            "run",
            "game.exe"
        ])
    );
}

#[test]
fn unsafe_ambiguities_and_expanded_empty_programs_fail() {
    assert!(build("command: ['${vars.EMPTY}']\nvariables: {EMPTY: ''}", &[]).is_err());
    assert!(
        build(
            "runtime: {mode: none}\ncompanions: [{name: helper, command: [true]}]",
            &["/tmp/_v2-entry-point", "--", "game"]
        )
        .is_err()
    );
    assert!(
        build(
            "runtime: {command: [runtime]}",
            &["/tmp/_v2-entry-point", "--", "game"]
        )
        .is_err()
    );
    assert!(config::parse("companions: [{name: helper, command: [helper.exe], proton: true, env: {STEAM_COMPAT_DATA_PATH: /another}}]").is_err());
}

#[test]
fn upstream_launcher_service_layout_is_recognized() {
    let args = strings(&[
        "/runtime/steam-runtime-launcher-interface-0",
        "container-runtime",
        "/proton/proton",
        "run",
        "game.exe",
    ]);
    assert_eq!(plan::runtime_boundary(&args).unwrap(), Some(2));
}

#[test]
fn shipped_examples_validate() {
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/examples")).unwrap() {
        let path = entry.unwrap().path();
        config::parse(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
    }
}
#[test]
fn atomic_save_preserves_original_on_validation_and_conflict() {
    let dir = std::env::temp_dir().join(format!("gsr-storage-{}", storage::nonce()));
    storage::private_dir(&dir).unwrap();
    let path = dir.join("demo.yaml");
    storage::create_once(&path, b"version: 1\n").unwrap();
    storage::create_once(&path, b"not valid").unwrap();
    assert!(storage::save_config(&path, "wrong: 1", "version: 1\n").is_err());
    assert!(storage::save_config(&path, "version: 1\nname: new\n", "stale").is_err());
    assert_eq!(storage::read_text(&path).unwrap(), "version: 1\n");
    storage::save_config(&path, "version: 1\nname: new\n", "version: 1\n").unwrap();
    assert_eq!(
        storage::read_text(&path.with_extension("yaml.bak")).unwrap(),
        "version: 1\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
