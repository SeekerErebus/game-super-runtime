use crate::config::{self, Config, MAX_CONFIG_BYTES};
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
}
impl Paths {
    pub fn discover(override_dir: Option<PathBuf>) -> Result<Self> {
        fn base(var: &str, fallback: &str) -> Result<PathBuf> {
            if let Some(value) = env::var_os(var).filter(|v| !v.is_empty()) {
                let path = PathBuf::from(value);
                ensure!(path.is_absolute(), "{var} must be an absolute path");
                return Ok(path);
            }
            let home = env::var_os("HOME")
                .context("HOME is not set; set HOME or the XDG directory variables")?;
            ensure!(
                Path::new(&home).is_absolute(),
                "HOME must be an absolute path"
            );
            Ok(PathBuf::from(home).join(fallback))
        }
        let config = override_dir
            .or_else(|| env::var_os("GSR_CONFIG_DIR").map(PathBuf::from))
            .map(Ok)
            .unwrap_or_else(|| {
                base("XDG_CONFIG_HOME", ".config").map(|p| p.join("game-super-runtime"))
            })?;
        let config = if config.is_absolute() {
            config
        } else {
            env::current_dir()?.join(config)
        };
        let state = base("XDG_STATE_HOME", ".local/state")?.join("game-super-runtime");
        Ok(Self { config, state })
    }
    pub fn app(&self, id: &str) -> PathBuf {
        self.config.join(format!("{id}.yaml"))
    }
    pub fn defaults(&self) -> PathBuf {
        self.config.join("defaults.yaml")
    }
    pub fn ensure_config(&self, id: &str) -> Result<PathBuf> {
        config::valid_id(id)?;
        let path = self.app(id);
        if path.exists() {
            return Ok(path);
        }
        let text = if self.defaults().exists() {
            read_text(&self.defaults())?
        } else {
            DEFAULT_CONFIG.to_owned()
        };
        config::parse(&text).context("cannot create app config: defaults.yaml is invalid")?;
        create_once(&path, text.as_bytes())?;
        Ok(path)
    }
    pub fn ensure_defaults(&self) -> Result<PathBuf> {
        let path = self.defaults();
        create_once(&path, DEFAULT_CONFIG.as_bytes())?;
        Ok(path)
    }
}

pub const DEFAULT_CONFIG: &str = "# Game Super Runtime\n# Empty settings preserve Steam's command. See the bundled docs/reference.md.\nversion: 1\n";

pub fn read_text(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    ensure!(
        file.metadata()?.is_file(),
        "{} is not a regular file",
        path.display()
    );
    let mut text = String::new();
    file.take(MAX_CONFIG_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .with_context(|| format!("cannot read UTF-8 configuration {}", path.display()))?;
    ensure!(
        text.len() <= MAX_CONFIG_BYTES,
        "{} exceeds 1 MiB",
        path.display()
    );
    Ok(text)
}
pub fn read_config(path: &Path) -> Result<Config> {
    config::parse(&read_text(path)?).with_context(|| format!("{}", path.display()))
}
pub fn private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .with_context(|| format!("cannot create directory {}", path.display()))
}
pub fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
pub fn create_once(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("configuration has no parent directory")?;
    private_dir(parent)?;
    let temp = parent.join(format!(".gsr-{}.tmp", nonce()));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        f.write_all(data)?;
        f.sync_all()?;
        match fs::hard_link(&temp, path) {
            Ok(()) => {
                File::open(parent)?.sync_all()?;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e).with_context(|| format!("cannot create {}", path.display())),
        }
    })();
    let _ = fs::remove_file(temp);
    result
}

/// Write after validation. Preserve the previous contents; reject stale browser edits.
pub fn save_config(path: &Path, text: &str, expected: &str) -> Result<()> {
    config::parse(text)?;
    let parent = path
        .parent()
        .context("configuration has no parent directory")?;
    let _lock = FileLock::acquire(&path.with_extension("yaml.lock"))?;
    ensure!(
        !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "GUI saving through a symlink is disabled; edit the target with --edit"
    );
    let old = read_text(path)?;
    ensure!(
        old == expected,
        "configuration changed on disk; reload before saving to avoid overwriting another edit"
    );
    atomic_write(&path.with_extension("yaml.bak"), old.as_bytes())?;
    atomic_write(path, text.as_bytes())?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", nonce()));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        f.write_all(data)?;
        f.sync_all()?;
        fs::rename(&temp, path).with_context(|| format!("cannot replace {}", path.display()))?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}

pub struct FileLock(File);
impl FileLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        use std::os::fd::AsRawFd;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .with_context(|| format!("cannot open lock {}", path.display()))?;
        // SAFETY: flock operates on an owned, valid descriptor. Closing releases it.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!(
                "{} is busy (another launch/editor may be running): {}",
                path.display(),
                std::io::Error::last_os_error()
            );
        }
        Ok(Self(file))
    }
}
impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.0.sync_all();
    }
}
