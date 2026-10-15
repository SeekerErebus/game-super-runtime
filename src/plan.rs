use crate::config::{Action, Config, ProtonVerb, RuntimeMode, Scope};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub argv: Vec<OsString>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub scope: Scope,
    pub allow_failure: bool,
    pub timeout_ms: Option<u64>,
}
#[derive(Debug, Clone)]
pub struct CompanionPlan {
    pub task: Task,
    pub required: bool,
    pub start_delay_ms: u64,
    pub start_after_game: bool,
    pub ready_file: Option<PathBuf>,
    pub ready_timeout_ms: u64,
}
#[derive(Debug, Clone)]
pub struct Plan {
    pub app_id: String,
    pub game: Vec<OsString>,
    pub runtime: Vec<OsString>,
    pub wrappers: Vec<Vec<OsString>>,
    pub env: BTreeMap<String, String>,
    pub unset_env: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub before: Vec<Task>,
    pub after: Vec<Task>,
    pub companions: Vec<CompanionPlan>,
    pub shutdown_timeout_ms: u64,
    pub warnings: Vec<String>,
    pub proton: Option<PathBuf>,
    pub prefix: Option<PathBuf>,
}

pub struct Expansion {
    builtin: BTreeMap<String, String>,
    variables: BTreeMap<String, String>,
    inherited: BTreeMap<String, String>,
}
impl Expansion {
    pub fn new(
        app_id: &str,
        config_dir: &Path,
        variables: &BTreeMap<String, String>,
    ) -> Result<Self> {
        let mut builtin = BTreeMap::new();
        builtin.insert("APP_ID".into(), app_id.into());
        builtin.insert(
            "CONFIG_DIR".into(),
            config_dir
                .to_str()
                .context("config directory must be UTF-8")?
                .into(),
        );
        builtin.insert(
            "HOME".into(),
            env::var("HOME").context("HOME must be set to a UTF-8 path")?,
        );
        Ok(Self {
            builtin,
            variables: variables.clone(),
            inherited: env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
        })
    }
    pub fn expand(&self, text: &str) -> Result<String> {
        self.expand_inner(text, &mut Vec::new())
    }
    fn expand_inner(&self, text: &str, stack: &mut Vec<String>) -> Result<String> {
        ensure!(stack.len() < 32, "variable expansion exceeds 32 levels");
        let mut output = String::new();
        let mut rest = text;
        while let Some(pos) = rest.find('$') {
            output.push_str(&rest[..pos]);
            rest = &rest[pos..];
            if rest.starts_with("$$") {
                output.push('$');
                rest = &rest[2..];
                continue;
            }
            if !rest.starts_with("${") {
                output.push('$');
                rest = &rest[1..];
                continue;
            }
            let end = rest
                .find('}')
                .context("unterminated ${...} variable; use $$ for a literal dollar")?;
            let key = &rest[2..end];
            if let Some(name) = key.strip_prefix("vars.") {
                ensure!(
                    !stack.iter().any(|x| x == name),
                    "variable cycle involving {name}"
                );
                let value = self
                    .variables
                    .get(name)
                    .with_context(|| format!("unknown variable ${{{key}}}"))?;
                stack.push(name.into());
                output.push_str(&self.expand_inner(value, stack)?);
                stack.pop();
            } else if let Some(name) = key.strip_prefix("env.") {
                output.push_str(
                    self.inherited
                        .get(name)
                        .with_context(|| format!("environment variable {name} is not set"))?,
                );
            } else {
                output.push_str(self.builtin.get(key).with_context(|| format!("unknown variable ${{{key}}}; use ${{vars.NAME}}, ${{env.NAME}}, APP_ID, HOME or CONFIG_DIR"))?);
            }
            ensure!(output.len() <= 1024 * 1024, "expanded value exceeds 1 MiB");
            rest = &rest[end + 1..];
        }
        output.push_str(rest);
        ensure!(!output.contains('\0'), "expanded value contains NUL");
        Ok(output)
    }
    pub fn path(&self, text: &str) -> Result<PathBuf> {
        let text = self.expand(text)?;
        ensure!(!text.is_empty(), "path must not be empty");
        let text = if text == "~" {
            self.builtin["HOME"].clone()
        } else if let Some(tail) = text.strip_prefix("~/") {
            format!("{}/{tail}", self.builtin["HOME"])
        } else {
            text
        };
        let path = PathBuf::from(text);
        Ok(if path.is_absolute() {
            path
        } else {
            Path::new(&self.builtin["CONFIG_DIR"]).join(path)
        })
    }
    fn argv(&self, argv: &[String]) -> Result<Vec<OsString>> {
        let result = self.words(argv)?;
        ensure!(
            result.first().is_none_or(|s| !s.is_empty()),
            "expanded executable must not be empty"
        );
        Ok(result)
    }
    fn words(&self, argv: &[String]) -> Result<Vec<OsString>> {
        argv.iter()
            .map(|x| self.expand(x).map(OsString::from))
            .collect()
    }
    fn env(&self, values: &BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
        values
            .iter()
            .map(|(k, v)| Ok((k.clone(), self.expand(v)?)))
            .collect()
    }
}

// Only recognize executable positions reached through known Steam wrappers.
// Searching for a word anywhere in argv would mistake game arguments for a runtime.
pub fn runtime_boundary(argv: &[OsString]) -> Result<Option<usize>> {
    let mut i = 0;
    for _ in 0..8 {
        let Some(exe) = argv.get(i) else {
            return Ok(None);
        };
        let name = Path::new(exe)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("");
        match name {
            "reaper" | "steam-launch-wrapper" => {
                let Some(end) = argv[i + 1..].iter().position(|s| s == "--") else {
                    return Ok(None);
                };
                i += end + 2;
            }
            "_v2-entry-point" | "pressure-vessel-wrap" => {
                let end = argv[i + 1..]
                    .iter()
                    .position(|s| s == "--")
                    .context("runtime command needs an explicit -- before the game")?
                    + i
                    + 2;
                ensure!(end < argv.len(), "runtime command has no game after --");
                return Ok(Some(end));
            }
            "steam-runtime-launcher-interface-0"
                if argv.get(i + 1).is_some_and(|s| s == "container-runtime") =>
            {
                ensure!(i + 2 < argv.len(), "container-runtime has no command");
                return Ok(Some(i + 2));
            }
            "run"
                if Path::new(exe)
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(OsStr::to_str)
                    .is_some_and(|s| s.starts_with("SteamLinuxRuntime")) =>
            {
                let end = argv[i + 1..]
                    .iter()
                    .position(|s| s == "--")
                    .context("SteamLinuxRuntime/run needs --")?
                    + i
                    + 2;
                ensure!(end < argv.len(), "runtime command has no game");
                return Ok(Some(end));
            }
            _ => return Ok(None),
        }
    }
    bail!("too many nested Steam wrappers; set runtime.command explicitly")
}

fn proton_index(argv: &[OsString]) -> Option<usize> {
    let mut i = 0;
    for _ in 0..8 {
        let name = Path::new(argv.get(i)?).file_name()?.to_str()?;
        if name == "proton"
            && argv
                .get(i + 1)
                .and_then(|s| s.to_str())
                .is_some_and(|s| matches!(s, "run" | "waitforexitandrun" | "runinprefix"))
        {
            return Some(i);
        }
        if matches!(name, "reaper" | "steam-launch-wrapper") {
            i += argv[i + 1..].iter().position(|s| s == "--")? + 2;
        } else {
            return None;
        }
    }
    None
}

pub fn build(
    config: &Config,
    app_id: &str,
    config_dir: &Path,
    supplied: &[OsString],
) -> Result<Plan> {
    config.validate()?;
    let expansion = Expansion::new(app_id, config_dir, &config.variables)?;
    let mut game = if supplied.is_empty() {
        expansion.argv(&config.command)?
    } else {
        supplied.to_vec()
    };
    ensure!(
        !game.is_empty(),
        "no launch command: launch through Steam with game-super-runtime {app_id} %command%, or set command: [executable, argument] in {app_id}.yaml"
    );
    ensure!(!game[0].is_empty(), "launch executable is empty");
    let mut warnings = vec![];
    let mut runtime = vec![];
    if !config.runtime.command.is_empty() {
        ensure!(
            runtime_boundary(&game)?.is_none(),
            "runtime.command and the supplied Steam runtime would nest containers; remove runtime.command when using Steam's runtime"
        );
        runtime = expansion.argv(&config.runtime.command)?;
    } else if config.runtime.mode != RuntimeMode::None
        && (config.runtime.mode == RuntimeMode::Required
            || config.proton.is_some()
            || !config.game_wrappers.is_empty()
            || config
                .before
                .iter()
                .chain(&config.after)
                .any(|a| a.scope == Scope::Game)
            || config.companions.iter().any(|c| c.scope == Scope::Game))
    {
        if let Some(index) = runtime_boundary(&game)? {
            runtime = game[..index].to_vec();
            game = game[index..].to_vec();
        }
    }
    ensure!(
        config.runtime.mode != RuntimeMode::Required || !runtime.is_empty(),
        "runtime.mode: required needs a recognized Steam container command or runtime.command"
    );

    let mut env = expansion.env(&config.env)?;
    if config.runtime.mode == RuntimeMode::None && runtime_boundary(&game)?.is_some() {
        ensure!(
            config.companions.iter().all(|c| c.scope == Scope::Host)
                && config
                    .before
                    .iter()
                    .chain(&config.after)
                    .all(|a| a.scope == Scope::Host)
                && config.game_wrappers.is_empty(),
            "runtime.mode: none hides the Steam container boundary; use auto for game-scope companions, hooks or wrappers"
        );
    }
    let mut index = proton_index(&game);
    let mut proton = index.map(|i| PathBuf::from(&game[i]));
    let mut prefix = env
        .get("STEAM_COMPAT_DATA_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            if config
                .unset_env
                .iter()
                .any(|s| s == "STEAM_COMPAT_DATA_PATH")
            {
                None
            } else {
                env::var_os("STEAM_COMPAT_DATA_PATH").map(PathBuf::from)
            }
        });
    if let Some(settings) = &config.proton {
        for (k, v) in expansion.env(&settings.env)? {
            env.insert(k, v);
        }
        if let Some(path) = env.get("STEAM_COMPAT_DATA_PATH") {
            prefix = Some(PathBuf::from(path));
        }
        if let Some(path) = &settings.path {
            let mut path = expansion.path(path)?;
            if path.is_dir() {
                path.push("proton");
            }
            proton = Some(path.clone());
            if let Some(i) = index {
                game[i] = path.into_os_string();
            } else {
                ensure!(
                    runtime_boundary(&game)?.is_none(),
                    "cannot wrap a complete container command in Proton; use runtime.mode: auto"
                );
                let mut cmd = vec![
                    path.into_os_string(),
                    OsString::from(
                        settings
                            .verb
                            .unwrap_or(ProtonVerb::Waitforexitandrun)
                            .as_str(),
                    ),
                ];
                cmd.extend(game);
                game = cmd;
                index = Some(0);
            }
        }
        if let Some(verb) = settings.verb {
            let i = index.context("proton.verb needs a Proton launch command or proton.path")?;
            game[i + 1] = verb.as_str().into();
        }
        if let Some(path) = &settings.prefix {
            let path = expansion.path(path)?;
            ensure!(
                path.file_name() != Some(OsStr::new("pfx")),
                "proton.prefix is the compatdata directory, not its pfx subdirectory"
            );
            env.insert(
                "STEAM_COMPAT_DATA_PATH".into(),
                path.to_string_lossy().into(),
            );
            prefix = Some(path);
        }
        if let Some(path) = &settings.steam_root {
            env.insert(
                "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                expansion.path(path)?.to_string_lossy().into(),
            );
        }
        if proton.is_some() {
            ensure!(
                prefix.is_some(),
                "Proton needs proton.prefix (compatdata directory) or STEAM_COMPAT_DATA_PATH from Steam"
            );
            if !env.contains_key("STEAM_COMPAT_CLIENT_INSTALL_PATH")
                && env::var_os("STEAM_COMPAT_CLIENT_INSTALL_PATH").is_none()
            {
                let candidate =
                    PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".steam/root");
                ensure!(
                    candidate.is_dir(),
                    "set proton.steam_root to your Steam installation"
                );
                env.insert(
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                    candidate.to_string_lossy().into(),
                );
            }
            if app_id.bytes().all(|c| c.is_ascii_digit()) {
                for key in ["SteamAppId", "SteamGameId", "STEAM_COMPAT_APP_ID"] {
                    if env::var_os(key).is_none() {
                        env.entry(key.into()).or_insert_with(|| app_id.into());
                    }
                }
            }
        }
    }
    let overlays = &config.overlays;
    if overlays.mangohud {
        env.entry("MANGOHUD".into()).or_insert("1".into());
    }
    if let Some(value) = &overlays.mangohud_config {
        env.entry("MANGOHUD_CONFIG".into())
            .or_insert(expansion.expand(value)?);
    }
    if overlays.vkbasalt {
        env.entry("ENABLE_VKBASALT".into()).or_insert("1".into());
    }
    if let Some(value) = &overlays.vkbasalt_config {
        env.entry("VKBASALT_CONFIG_FILE".into())
            .or_insert(expansion.path(value)?.to_string_lossy().into());
    }
    if let Some(value) = &overlays.dxvk_hud {
        env.entry("DXVK_HUD".into())
            .or_insert(expansion.expand(value)?);
    }

    let mut wrappers: Vec<Vec<OsString>> = config
        .wrappers
        .iter()
        .map(|w| expansion.argv(w))
        .collect::<Result<_>>()?;
    if let Some(args) = &overlays.gamescope {
        let mut wrapper = vec!["gamescope".into()];
        wrapper.extend(expansion.words(args)?);
        wrapper.push("--".into());
        wrappers.push(wrapper);
    }
    if overlays.gamemode {
        wrappers.push(vec!["gamemoderun".into()]);
    }

    let make_task = |a: &Action, default_name: String| -> Result<Task> {
        let argv = action_argv(config, &expansion, &a.command, &a.script, &a.args)?;
        Ok(Task {
            name: a.name.clone().unwrap_or(default_name),
            argv,
            env: expansion.env(&a.env)?,
            cwd: a
                .working_dir
                .as_ref()
                .map(|s| expansion.path(s))
                .transpose()?,
            scope: a.scope,
            allow_failure: a.allow_failure,
            timeout_ms: a.timeout_ms,
        })
    };
    let before = config
        .before
        .iter()
        .enumerate()
        .map(|(i, a)| make_task(a, format!("before[{i}]")))
        .collect::<Result<Vec<_>>>()?;
    let after = config
        .after
        .iter()
        .enumerate()
        .map(|(i, a)| make_task(a, format!("after[{i}]")))
        .collect::<Result<Vec<_>>>()?;
    let mut companions = vec![];
    for companion in &config.companions {
        let mut argv = action_argv(
            config,
            &expansion,
            &companion.command,
            &companion.script,
            &companion.args,
        )?;
        if companion.proton {
            let executable = proton.as_ref().context(
                "a Proton companion needs proton.path or a recognized Steam Proton command",
            )?;
            ensure!(
                prefix.is_some(),
                "a Proton companion needs proton.prefix or STEAM_COMPAT_DATA_PATH"
            );
            let mut full = vec![executable.as_os_str().to_owned(), OsString::from("run")];
            full.extend(argv);
            argv = full;
            if let Some(i) = index {
                if game.get(i + 1).is_some_and(|v| v == "waitforexitandrun") {
                    ensure!(
                        config.proton.as_ref().and_then(|p| p.verb)
                            != Some(ProtonVerb::Waitforexitandrun),
                        "proton.verb: waitforexitandrun waits for companions to exit; use run when sharing a prefix"
                    );
                    game[i + 1] = "run".into();
                    warnings.push("Using Proton run so the game does not wait for its companion's wineserver to exit.".into());
                }
            }
        }
        companions.push(CompanionPlan {
            task: Task {
                name: companion.name.clone(),
                argv,
                env: expansion.env(&companion.env)?,
                cwd: companion
                    .working_dir
                    .as_ref()
                    .map(|v| expansion.path(v))
                    .transpose()?,
                scope: companion.scope,
                allow_failure: !companion.required,
                timeout_ms: None,
            },
            required: companion.required,
            start_delay_ms: companion.start_delay_ms,
            start_after_game: companion.start_after_game,
            ready_file: companion
                .ready_file
                .as_ref()
                .map(|v| expansion.path(v))
                .transpose()?,
            ready_timeout_ms: companion.ready_timeout_ms,
        });
    }
    for wrapper in config.game_wrappers.iter().rev() {
        game = wrap(&expansion.argv(wrapper)?, game);
    }
    for name in config.variables.keys() {
        env.insert(
            format!("GSR_VAR_{name}"),
            expansion.expand(&format!("${{vars.{name}}}"))?,
        );
    }
    if !before.is_empty() || !after.is_empty() || !companions.is_empty() {
        env.insert("GSR_APP_ID".into(), app_id.into());
        env.insert("GSR_CONFIG_DIR".into(), config_dir.to_string_lossy().into());
        if let Some(p) = &prefix {
            env.insert("GSR_PREFIX".into(), p.to_string_lossy().into());
        }
        if let Some(p) = &proton {
            env.insert("GSR_PROTON".into(), p.to_string_lossy().into());
        }
    }
    for key in &config.unset_env {
        ensure!(
            !env.contains_key(key),
            "unset_env conflicts with a generated setting for {key}"
        );
    }
    if let Some(prefix) = &prefix {
        if config.proton.is_some() || config.companions.iter().any(|c| c.proton) {
            ensure!(
                prefix.is_absolute(),
                "STEAM_COMPAT_DATA_PATH must be an absolute, nonempty path; prefer proton.prefix"
            );
        }
    }
    Ok(Plan {
        app_id: app_id.into(),
        game,
        runtime,
        wrappers,
        env,
        unset_env: config.unset_env.clone(),
        cwd: config
            .working_dir
            .as_ref()
            .map(|p| expansion.path(p))
            .transpose()?,
        before,
        after,
        companions,
        shutdown_timeout_ms: config.shutdown_timeout_ms,
        warnings,
        proton,
        prefix,
    })
}

fn action_argv(
    config: &Config,
    expansion: &Expansion,
    command: &[String],
    script: &Option<String>,
    args: &[String],
) -> Result<Vec<OsString>> {
    if let Some(name) = script {
        let script = &config.scripts[name];
        let mut argv: Vec<OsString> = vec![
            "/bin/bash".into(),
            "--noprofile".into(),
            "--norc".into(),
            "-euo".into(),
            "pipefail".into(),
        ];
        if let Some(body) = &script.body {
            argv.extend([
                OsString::from("-c"),
                OsString::from(body),
                OsString::from(format!("gsr-script-{name}")),
            ]);
        } else {
            argv.push(
                expansion
                    .path(script.path.as_ref().unwrap())?
                    .into_os_string(),
            );
        }
        argv.extend(expansion.words(args)?);
        Ok(argv)
    } else {
        expansion.argv(command)
    }
}

pub fn wrap(wrapper: &[OsString], command: Vec<OsString>) -> Vec<OsString> {
    wrapper.iter().cloned().chain(command).collect()
}

impl Plan {
    pub fn supervised(&self) -> bool {
        !self.before.is_empty() || !self.after.is_empty() || !self.companions.is_empty()
    }
    pub fn json(&self) -> Value {
        fn argv(a: &[OsString]) -> Vec<String> {
            a.iter().map(|x| x.to_string_lossy().into_owned()).collect()
        }
        fn task(t: &Task) -> Value {
            json!({"name": t.name, "argv": argv(&t.argv), "env": t.env, "working_dir": t.cwd, "scope": t.scope, "allow_failure": t.allow_failure, "timeout_ms": t.timeout_ms})
        }
        json!({"app_id": self.app_id, "game": argv(&self.game), "runtime": argv(&self.runtime), "wrappers": self.wrappers.iter().map(|a| argv(a)).collect::<Vec<_>>(),
            "env": self.env, "unset_env": self.unset_env, "working_dir": self.cwd,
            "before": self.before.iter().map(task).collect::<Vec<_>>(), "after": self.after.iter().map(task).collect::<Vec<_>>(),
            "companions": self.companions.iter().map(|c| json!({"task": task(&c.task), "required": c.required, "start_delay_ms": c.start_delay_ms, "start_after_game": c.start_after_game, "ready_file": c.ready_file, "ready_timeout_ms": c.ready_timeout_ms})).collect::<Vec<_>>(),
            "shutdown_timeout_ms": self.shutdown_timeout_ms, "warnings": self.warnings,
            "note": "Argument arrays are executed literally. Environment values in this preview may be private; non-UTF-8 argv is displayed lossily."})
    }
}
