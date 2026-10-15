//! Optional filesystem views. Mounts never cover a host game/prefix directory.
use crate::{
    execute,
    plan::Expansion,
    storage::{self, FileLock, Paths},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn enabled() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Directory {
    Bind {
        source: String,
        target: String,
        #[serde(default = "enabled")]
        read_only: bool,
    },
    Overlay {
        target: String,
        write_dir: String,
        #[serde(default)]
        layers: Vec<Layer>,
        #[serde(default)]
        manifest: Option<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub path: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    layers: Vec<Layer>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedDirectory {
    pub target: PathBuf,
    pub source: Option<PathBuf>,
    pub read_only: bool,
    /// Low to high priority; the target's original content is always the base.
    pub layers: Vec<PathBuf>,
    pub write_dir: Option<PathBuf>,
}

pub fn validate(dirs: &[Directory]) -> Result<()> {
    ensure!(dirs.len() <= 64, "virtual_directories: at most 64 mappings");
    for dir in dirs {
        let (target, paths) = match dir {
            Directory::Bind { source, target, .. } => (target, vec![source]),
            Directory::Overlay {
                target,
                write_dir,
                layers,
                manifest,
            } => {
                ensure!(
                    manifest.is_none() || layers.is_empty(),
                    "virtual_directories: use layers or manifest, not both"
                );
                ensure!(
                    layers.len() <= 128,
                    "virtual_directories: at most 128 layers"
                );
                let mut paths = vec![write_dir];
                paths.extend(layers.iter().map(|l| &l.path));
                paths.extend(manifest.iter());
                (target, paths)
            }
        };
        for path in std::iter::once(target).chain(paths) {
            ensure!(
                !path.is_empty() && !path.contains('\0'),
                "virtual_directories: paths must be nonempty without NUL"
            );
        }
    }
    Ok(())
}

pub fn resolve(dirs: &[Directory], expansion: &Expansion) -> Result<Vec<ResolvedDirectory>> {
    dirs.iter()
        .map(|dir| match dir {
            Directory::Bind {
                source,
                target,
                read_only,
            } => Ok(ResolvedDirectory {
                target: expansion.path(target)?,
                source: Some(expansion.path(source)?),
                read_only: *read_only,
                layers: vec![],
                write_dir: None,
            }),
            Directory::Overlay {
                target,
                write_dir,
                layers,
                manifest,
            } => {
                let layers = if let Some(manifest) = manifest {
                    let path = expansion.path(manifest)?;
                    let text = storage::read_text(&path).with_context(|| {
                        format!("cannot read virtual directory manifest {}", path.display())
                    })?;
                    // Value parsing rejects duplicate keys, including nested layer fields.
                    let _: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)?;
                    let manifest: Manifest = serde_yaml_ng::from_str(&text)
                        .context("invalid virtual directory manifest")?;
                    ensure!(
                        manifest.version == 1,
                        "virtual directory manifest version must be 1"
                    );
                    ensure!(manifest.layers.len() <= 128, "manifest: at most 128 layers");
                    manifest
                        .layers
                        .into_iter()
                        .filter(|l| l.enabled)
                        .map(|l| {
                            ensure!(
                                !l.path.is_empty() && !l.path.contains('\0'),
                                "manifest layer path is empty or contains NUL"
                            );
                            let p = PathBuf::from(l.path);
                            Ok(if p.is_absolute() {
                                p
                            } else {
                                path.parent().unwrap().join(p)
                            })
                        })
                        .collect::<Result<Vec<_>>>()?
                } else {
                    layers
                        .iter()
                        .filter(|l| l.enabled)
                        .map(|l| expansion.path(&l.path))
                        .collect::<Result<Vec<_>>>()?
                };
                Ok(ResolvedDirectory {
                    target: expansion.path(target)?,
                    source: None,
                    read_only: false,
                    layers,
                    write_dir: Some(expansion.path(write_dir)?),
                })
            }
        })
        .collect()
}

/// Host candidates used for diagnostics before the namespace exists.
pub fn visible_paths(dirs: &[ResolvedDirectory], path: &Path) -> Vec<PathBuf> {
    for d in dirs {
        if let Ok(tail) = path.strip_prefix(&d.target) {
            if let Some(source) = &d.source {
                return vec![source.join(tail)];
            }
            return d
                .write_dir
                .iter()
                .map(|p| p.join("upper").join(tail))
                .chain(d.layers.iter().rev().map(|p| p.join(tail)))
                .chain(std::iter::once(path.to_owned()))
                .collect();
        }
    }
    vec![path.to_owned()]
}

fn directory(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_dir(),
        "virtual directory {} does not exist or is not a directory; initialize the prefix and create the target directory first",
        path.display()
    );
    fs::canonicalize(path).with_context(|| format!("cannot resolve {}", path.display()))
}
fn related(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
fn option_path(path: &Path) -> Result<&str> {
    let s = path.to_str().context("FUSE overlay paths must be UTF-8")?;
    ensure!(
        !s.contains([':', ',', '\\', '\n', '\r']),
        "FUSE overlay path contains unsupported delimiter (: , backslash or newline): {}",
        path.display()
    );
    Ok(s)
}
// Resolve existing ancestors too, so aliases cannot bypass overlap checks before creation.
fn prospective(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    ensure!(
        !path.is_symlink(),
        "dangling symlink in write_dir: {}",
        path.display()
    );
    let parent = path.parent().context("write_dir has no parent")?;
    ensure!(
        path.file_name().is_some()
            && !path
                .components()
                .any(|c| c == std::path::Component::ParentDir),
        "write_dir must not contain .."
    );
    Ok(prospective(parent)?.join(path.file_name().unwrap()))
}

/// Pure checks: doctor does not mount, create storage, or execute configured commands.
pub fn preflight(dirs: &[ResolvedDirectory]) -> Result<Vec<ResolvedDirectory>> {
    if dirs.is_empty() {
        return Ok(vec![]);
    }
    for tool in ["bwrap"]
        .into_iter()
        .chain(if dirs.iter().any(|d| d.source.is_none()) {
            vec!["fuse-overlayfs", "fusermount3"]
        } else {
            vec![]
        })
    {
        ensure!(
            execute::executable_exists(OsStr::new(tool), None, None),
            "virtual_directories requires {tool}; install bubblewrap (and fuse-overlayfs + fuse3 for overlays). Profiles without virtual_directories do not need these tools"
        );
    }
    if dirs.iter().any(|d| d.source.is_none()) {
        fs::OpenOptions::new().read(true).write(true).open("/dev/fuse").context("virtual_directories overlays require accessible /dev/fuse; check FUSE/device permissions")?;
    }
    let mut result = dirs.to_vec();
    for d in &mut result {
        d.target = directory(&d.target)?;
        ensure!(
            !d.target.as_os_str().as_encoded_bytes().contains(&b':'),
            "virtual directory target cannot contain : (Steam filesystem export separator)"
        );
        ensure!(
            d.target != Path::new("/")
                && ![
                    "/proc", "/sys", "/dev", "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64"
                ]
                .iter()
                .any(|p| d.target.starts_with(p)),
            "virtual directory target must be a game/prefix data directory: {}",
            d.target.display()
        );
        if let Some(source) = &mut d.source {
            *source = directory(source)?;
        }
        for layer in &mut d.layers {
            *layer = directory(layer)?;
        }
        if let Some(write) = &mut d.write_dir {
            *write = prospective(write)?;
            option_path(write)?;
            option_path(&d.target)?;
            for layer in &d.layers {
                option_path(layer)?;
            }
        }
    }
    for (i, d) in result.iter().enumerate() {
        for other in &result[i + 1..] {
            ensure!(
                !related(&d.target, &other.target),
                "virtual directory targets overlap: {} and {}",
                d.target.display(),
                other.target.display()
            );
        }
        for other in &result {
            for source in other.source.iter().chain(&other.layers) {
                ensure!(
                    !related(&d.target, source),
                    "virtual directory source and target overlap: {} and {}",
                    source.display(),
                    d.target.display()
                );
            }
            if let Some(write) = &d.write_dir {
                for data in std::iter::once(&other.target)
                    .chain(other.source.iter())
                    .chain(&other.layers)
                {
                    ensure!(
                        !related(write, data),
                        "overlay write_dir overlaps source/target: {}",
                        write.display()
                    );
                }
                if let Some(other_write) = &other.write_dir {
                    ensure!(
                        std::ptr::eq(d, other) || !related(write, other_write),
                        "overlay write directories overlap"
                    );
                }
            }
        }
    }
    Ok(result)
}

struct Mount {
    child: Child,
    path: PathBuf,
}
pub struct Session {
    mounts: Vec<Mount>,
    locks: Vec<FileLock>,
    root: PathBuf,
    pub prefix: Vec<OsString>,
    targets: Vec<PathBuf>,
}
impl Session {
    pub fn prepare(dirs: &[ResolvedDirectory], paths: &Paths) -> Result<Option<Self>> {
        if dirs.is_empty() {
            return Ok(None);
        }
        let dirs = preflight(dirs)?;
        let root = paths.state.join("virtual-sessions").join(storage::nonce());
        for d in &dirs {
            for data in std::iter::once(&d.target)
                .chain(d.source.iter())
                .chain(&d.layers)
                .chain(d.write_dir.iter())
            {
                ensure!(
                    !related(&prospective(&root)?, data),
                    "virtual directory overlaps launcher mount storage: {}",
                    data.display()
                );
            }
        }
        storage::private_dir(&root)?;
        let mut session = Self {
            targets: dirs.iter().map(|d| d.target.clone()).collect(),
            mounts: vec![],
            locks: vec![],
            root,
            prefix: ["bwrap", "--die-with-parent", "--dev-bind", "/", "/"]
                .map(OsString::from)
                .to_vec(),
        };
        session.prefix[0] = std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
        )
        .map(|p| p.join("bwrap"))
        .find(|p| execute::executable_exists(p.as_os_str(), None, None))
        .context("cannot locate bwrap")?
        .canonicalize()?
        .into_os_string();
        for (i, d) in dirs.iter().enumerate() {
            ensure!(
                execute::received_signal() == 0,
                "virtual directory setup interrupted"
            );
            let source = if let Some(source) = &d.source {
                source.clone()
            } else {
                let write = d.write_dir.as_ref().unwrap();
                storage::private_dir(write)?;
                session
                    .locks
                    .push(FileLock::acquire(&write.join(".gsr.lock"))?);
                // Only these two directories belong to the overlay; never delete user storage.
                let upper = write.join("upper");
                let work = write.join("work");
                for path in [&upper, &work] {
                    ensure!(
                        !path.is_symlink(),
                        "overlay upper/work must not be symlinks"
                    );
                    storage::private_dir(path)?;
                }
                let mount = session.root.join(i.to_string());
                storage::private_dir(&mount)?;
                let lowers = d
                    .layers
                    .iter()
                    .rev()
                    .chain(std::iter::once(&d.target))
                    .map(|p| option_path(p))
                    .collect::<Result<Vec<_>>>()?
                    .join(":");
                let options = format!(
                    "lowerdir={lowers},upperdir={},workdir={}",
                    option_path(&upper)?,
                    option_path(&work)?
                );
                let mut cmd = host_command("fuse-overlayfs");
                cmd.args(["-f", "-o", &options])
                    .arg(&mount)
                    .stdin(Stdio::null())
                    .process_group(0);
                // Kill the foreground service even if GSR is forcibly terminated.
                unsafe {
                    cmd.pre_exec(|| {
                        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        if libc::getppid() == 1 {
                            return Err(std::io::Error::other("launcher exited"));
                        }
                        Ok(())
                    });
                }
                let child = cmd
                    .spawn()
                    .inspect_err(|_| {
                        let _ = fs::remove_dir(&mount);
                    })
                    .context("cannot start fuse-overlayfs")?;
                session.mounts.push(Mount {
                    child,
                    path: mount.clone(),
                });
                let start = Instant::now();
                loop {
                    ensure!(
                        session
                            .mounts
                            .last_mut()
                            .unwrap()
                            .child
                            .try_wait()?
                            .is_none(),
                        "fuse-overlayfs exited before mounting {}; see its error above",
                        mount.display()
                    );
                    if mounted(&mount)? {
                        break;
                    }
                    ensure!(
                        execute::received_signal() == 0,
                        "virtual directory setup interrupted"
                    );
                    ensure!(
                        start.elapsed() < Duration::from_secs(10),
                        "FUSE mount timed out: {}",
                        mount.display()
                    );
                    thread::sleep(Duration::from_millis(20));
                }
                mount
            };
            session
                .prefix
                .push(if d.read_only { "--ro-bind" } else { "--bind" }.into());
            session
                .prefix
                .extend([source.into_os_string(), d.target.as_os_str().to_owned()]);
        }
        session.prefix.push("--".into());
        Ok(Some(session))
    }
    pub fn exports(&self, inherited: &str) -> String {
        std::iter::once(inherited.to_owned())
            .filter(|s| !s.is_empty())
            .chain(
                self.targets
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned()),
            )
            .collect::<Vec<_>>()
            .join(":")
    }
    pub fn healthy(&mut self) -> Result<()> {
        for mount in &mut self.mounts {
            ensure!(
                mount.child.try_wait()?.is_none(),
                "FUSE service stopped for {}",
                mount.path.display()
            );
        }
        Ok(())
    }
}
fn host_command(tool: &str) -> Command {
    let mut cmd = Command::new(tool);
    // These utilities run on the host, even when launched by Steam.
    if let Some(path) = std::env::var_os("SYSTEM_LD_LIBRARY_PATH") {
        cmd.env("LD_LIBRARY_PATH", path);
    } else {
        cmd.env_remove("LD_LIBRARY_PATH");
    }
    cmd.env_remove("LD_PRELOAD");
    cmd
}
fn mounted(path: &Path) -> Result<bool> {
    // mountinfo escapes whitespace and backslashes in mountpoint names.
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\134")
        .replace(' ', "\\040")
        .replace('\t', "\\011")
        .replace('\n', "\\012");
    Ok(fs::read_to_string("/proc/self/mountinfo")?
        .lines()
        .any(|line| line.split_whitespace().nth(4) == Some(escaped.as_str())))
}
impl Drop for Session {
    fn drop(&mut self) {
        for mount in self.mounts.iter_mut().rev() {
            let _ = host_command("fusermount3")
                .args(["-u", "-z", "--"])
                .arg(&mount.path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = mount.child.kill();
            let _ = mount.child.wait();
            // Never recursively remove a mount: failed unmount must not delete data.
            if mounted(&mount.path).is_ok_and(|v| !v) {
                let _ = fs::remove_dir(&mount.path);
            } else {
                eprintln!(
                    "game-super-runtime: could not confirm unmount of {}; inspect it before removing the empty session directory (see virtual-directories.md)",
                    mount.path.display()
                );
            }
        }
        let _ = fs::remove_dir(&self.root);
    }
}
