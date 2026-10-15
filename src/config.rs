use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub name: Option<String>,
    pub command: Vec<String>,
    pub working_dir: Option<String>,
    pub variables: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
    pub unset_env: Vec<String>,
    pub wrappers: Vec<Vec<String>>,
    pub game_wrappers: Vec<Vec<String>>,
    pub overlays: Overlays,
    pub proton: Option<Proton>,
    pub runtime: Runtime,
    pub scripts: BTreeMap<String, Script>,
    pub before: Vec<Action>,
    pub after: Vec<Action>,
    pub companions: Vec<Companion>,
    pub virtual_directories: Vec<crate::virtual_dirs::Directory>,
    pub shutdown_timeout_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            name: None,
            command: vec![],
            working_dir: None,
            variables: BTreeMap::new(),
            env: BTreeMap::new(),
            unset_env: vec![],
            wrappers: vec![],
            game_wrappers: vec![],
            overlays: Overlays::default(),
            proton: None,
            runtime: Runtime::default(),
            scripts: BTreeMap::new(),
            before: vec![],
            after: vec![],
            companions: vec![],
            virtual_directories: vec![],
            shutdown_timeout_ms: 3000,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Overlays {
    pub gamemode: bool,
    pub mangohud: bool,
    pub mangohud_config: Option<String>,
    pub gamescope: Option<Vec<String>>,
    pub vkbasalt: bool,
    pub vkbasalt_config: Option<String>,
    pub dxvk_hud: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Proton {
    pub path: Option<String>,
    pub prefix: Option<String>,
    pub steam_root: Option<String>,
    pub verb: Option<ProtonVerb>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProtonVerb {
    Run,
    Waitforexitandrun,
    Runinprefix,
}
impl ProtonVerb {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Waitforexitandrun => "waitforexitandrun",
            Self::Runinprefix => "runinprefix",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Runtime {
    pub mode: RuntimeMode,
    pub command: Vec<String>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    #[default]
    Auto,
    Required,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub body: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Action {
    pub name: Option<String>,
    pub command: Vec<String>,
    pub script: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<String>,
    pub scope: Scope,
    pub allow_failure: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Host,
    #[default]
    Game,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Companion {
    pub name: String,
    pub command: Vec<String>,
    pub script: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<String>,
    pub scope: Scope,
    pub proton: bool,
    pub host_environment: bool,
    pub required: bool,
    pub start_delay_ms: u64,
    pub start_after_game: bool,
    pub ready_file: Option<String>,
    pub ready_timeout_ms: u64,
}
impl Default for Companion {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: vec![],
            script: None,
            args: vec![],
            env: BTreeMap::new(),
            working_dir: None,
            scope: Scope::Game,
            proton: false,
            host_environment: false,
            required: false,
            start_delay_ms: 0,
            start_after_game: false,
            ready_file: None,
            ready_timeout_ms: 10000,
        }
    }
}

pub fn parse(text: &str) -> Result<Config> {
    ensure!(
        text.len() <= MAX_CONFIG_BYTES,
        "YAML exceeds the 1 MiB configuration limit"
    );
    // Parse a Value first to reject duplicate keys even inside otherwise free-form maps.
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).context("invalid YAML")?;
    if value.is_null() {
        return Ok(Config::default());
    }
    ensure!(value.is_mapping(), "configuration must be a YAML mapping");
    let config: Config = serde_yaml_ng::from_str(text).context("invalid configuration")?;
    config.validate()?;
    Ok(config)
}

pub fn valid_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id != "."
            && id != ".."
            && !id.starts_with('.')
            && id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)),
        "APP-ID must be 1–128 letters, digits, dots, underscores or hyphens, and cannot start with a dot"
    );
    ensure!(
        id != "defaults",
        "APP-ID 'defaults' is reserved; use --edit-defaults to edit the template"
    );
    Ok(())
}

pub fn valid_env(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn check_env(env: &BTreeMap<String, String>, field: &str) -> Result<()> {
    for (name, value) in env {
        ensure!(
            valid_env(name),
            "{field}: invalid environment name {name:?}"
        );
        ensure!(
            !name.starts_with("GSR_"),
            "{field}: GSR_ names are reserved ({name})"
        );
        ensure!(!value.contains('\0'), "{field}.{name}: NUL is not allowed");
    }
    Ok(())
}
fn check_command(command: &[String], field: &str, optional: bool) -> Result<()> {
    ensure!(
        optional || !command.is_empty(),
        "{field}: command must not be empty"
    );
    if let Some(first) = command.first() {
        ensure!(!first.is_empty(), "{field}: executable must not be empty");
    }
    ensure!(
        command.iter().all(|s| !s.contains('\0')),
        "{field}: NUL is not allowed"
    );
    Ok(())
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.version == 1,
            "unsupported configuration version {}; expected 1",
            self.version
        );
        check_command(&self.command, "command", true)?;
        crate::virtual_dirs::validate(&self.virtual_directories)?;
        check_env(&self.env, "env")?;
        check_env(&self.variables, "variables")?;
        ensure!(
            self.shutdown_timeout_ms <= 60000,
            "shutdown_timeout_ms must be at most 60000"
        );
        for key in &self.unset_env {
            ensure!(
                valid_env(key) && !key.starts_with("GSR_"),
                "unset_env: invalid or reserved name {key:?}"
            );
            ensure!(
                !self.env.contains_key(key),
                "{key} occurs in both env and unset_env"
            );
        }
        for (field, wrappers) in [
            ("wrappers", &self.wrappers),
            ("game_wrappers", &self.game_wrappers),
        ] {
            for command in wrappers {
                check_command(command, field, false)?;
            }
        }
        check_command(&self.runtime.command, "runtime.command", true)?;
        ensure!(
            self.runtime.mode != RuntimeMode::None || self.runtime.command.is_empty(),
            "runtime.command cannot be combined with runtime.mode: none"
        );
        if let Some(proton) = &self.proton {
            check_env(&proton.env, "proton.env")?;
        }
        for (name, script) in &self.scripts {
            ensure!(valid_env(name), "invalid script name {name:?}");
            ensure!(
                script.body.is_some() != script.path.is_some(),
                "scripts.{name}: specify exactly one of body or path"
            );
            ensure!(
                !script
                    .body
                    .as_deref()
                    .or(script.path.as_deref())
                    .unwrap_or("")
                    .is_empty(),
                "scripts.{name} is empty"
            );
        }
        for (field, actions) in [("before", &self.before), ("after", &self.after)] {
            for (index, action) in actions.iter().enumerate() {
                let label = format!("{field}[{index}]");
                self.check_action(&action.command, &action.script, &action.args, &label)?;
                check_env(&action.env, &label)?;
                if let Some(ms) = action.timeout_ms {
                    ensure!(
                        (1..=86400000).contains(&ms),
                        "{label}.timeout_ms must be 1..86400000"
                    );
                }
            }
        }
        let mut names = BTreeSet::new();
        for companion in &self.companions {
            let label = format!("companions.{}", companion.name);
            ensure!(
                !companion.name.is_empty() && names.insert(&companion.name),
                "companions need nonempty, unique names"
            );
            self.check_action(
                &companion.command,
                &companion.script,
                &companion.args,
                &label,
            )?;
            check_env(&companion.env, &label)?;
            ensure!(
                !companion.host_environment || companion.scope == Scope::Host,
                "{label}: host_environment requires scope: host"
            );
            ensure!(
                !companion.proton || companion.script.is_none(),
                "{label}: proton requires a command, not a Bash script"
            );
            ensure!(
                !companion.proton || companion.scope == Scope::Game,
                "{label}: Proton companions must use scope: game to share the runtime"
            );
            ensure!(
                !companion.proton
                    || !companion
                        .env
                        .keys()
                        .any(|k| matches!(k.as_str(), "STEAM_COMPAT_DATA_PATH" | "WINEPREFIX")),
                "{label}: Proton companions share the game's prefix; remove the per-companion prefix override"
            );
            ensure!(
                companion.start_delay_ms <= 3600000,
                "{label}.start_delay_ms must be at most 3600000"
            );
            ensure!(
                (1..=3600000).contains(&companion.ready_timeout_ms),
                "{label}.ready_timeout_ms must be 1..3600000"
            );
        }
        // Reject NUL throughout the object, including names, script bodies and paths.
        fn no_nul(value: &serde_json::Value) -> bool {
            match value {
                serde_json::Value::String(s) => !s.contains('\0'),
                serde_json::Value::Array(a) => a.iter().all(no_nul),
                serde_json::Value::Object(o) => {
                    o.iter().all(|(k, v)| !k.contains('\0') && no_nul(v))
                }
                _ => true,
            }
        }
        ensure!(
            no_nul(&serde_json::to_value(self)?),
            "configuration contains a NUL byte"
        );
        Ok(())
    }
    fn check_action(
        &self,
        cmd: &[String],
        script: &Option<String>,
        args: &[String],
        label: &str,
    ) -> Result<()> {
        if cmd.is_empty() == script.is_none() {
            bail!("{label}: specify exactly one of command or script");
        }
        check_command(cmd, label, script.is_some())?;
        if let Some(script) = script {
            ensure!(
                self.scripts.contains_key(script),
                "{label}: unknown script {script:?}"
            );
        } else {
            ensure!(
                args.is_empty(),
                "{label}: args belongs to a script; put command arguments in command"
            );
        }
        Ok(())
    }
}
