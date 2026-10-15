# Virtual directories

`virtual_directories` builds a private filesystem view before launch wrappers,
hooks, Proton, the game and companions start. Native programs can use it too.
Two mapping types are supported: shared directory binds and ordered mod overlays.
The host's target directories are never replaced, renamed or symlinked by GSR.
The editor has a **Virtual directories** tab; Full YAML exposes the same settings.

## Setup and optional dependencies

Plain profiles still need no mount tools. Bind mappings require `bubblewrap`.
Overlays additionally require `fuse-overlayfs`, `fusermount3` (FUSE 3), and readable/
writable `/dev/fuse`. Ubuntu packages:

```sh
sudo apt install bubblewrap fuse-overlayfs fuse3
```

Arch packages have the same names. Use distribution packages, not a root launch.
Missing tools/device access produce an error before the game starts. `--doctor`
checks dependencies and source/target layout without mounting or launching tools;
`--dry-run` resolves paths and manifests without creating overlay storage. Actual
namespace/mount permissions are checked when launching. Backend stderr remains
attached to the terminal/Steam; GSR records setup/failure in its lifecycle log.

Ubuntu supports FUSE, but device access and permission to create nested user/mount
namespaces are separate requirements. Ubuntu 24.04 introduced AppArmor restrictions
on unprivileged user namespaces. Use the packaged bubblewrap/Steam profiles and
consult denied-operation logs if bwrap reports permission errors. Flatpak/Snap
must expose all paths and tools, `/dev/fuse`, and permit the needed nesting; these
packaging environments have not been verified. Do not assume installing FUSE
alone resolves namespace restrictions.

References: [fuse-overlayfs](https://github.com/containers/fuse-overlayfs),
[bubblewrap](https://github.com/containers/bubblewrap), and
[Ubuntu user namespace restrictions](https://discourse.ubuntu.com/t/understanding-apparmor-user-namespace-restriction/58007).

## Shared application and app-data folders

Initialize the game prefix first, locate the installed application's actual
folders, and create the corresponding empty **target directories** if absent.
GSR requires all source and target directories to exist. It resolves symlinks,
including prefixes whose `pfx` points to `.`, before mounting. Map narrow app
folders, not the entire prefix, drive, or Windows user profile.

```yaml
version: 1
virtual_directories:
  - type: bind
    source: '~/Games/Prefixes/WeMod/pfx/drive_c/users/steamuser/AppData/Local/WeMod'
    target: '${PREFIX}/pfx/drive_c/users/steamuser/AppData/Local/WeMod'
    read_only: true
  - type: bind
    source: '~/Games/Prefixes/WeMod/pfx/drive_c/users/steamuser/AppData/Roaming/WeMod'
    target: '${PREFIX}/pfx/drive_c/users/steamuser/AppData/Roaming/WeMod'
    read_only: false
companions:
  - name: Shared companion
    proton: true
    command: ['C:\users\steamuser\AppData\Local\WeMod\WeMod.exe']
    start_after_game: true
```

Paths above illustrate a layout, not a verified WeMod installation recipe. Check
its actual executable version/layout and Windows username. `${PREFIX}` is the
resolved game's **compatdata directory**, from `proton.prefix` or Steam's
`STEAM_COMPAT_DATA_PATH`. It is available in virtual-directory fields and task
fields evaluated after Proton selection; do not use it to define `proton.prefix`
or the initial game command. Ordinary `${vars.NAME}`, `${env.NAME}`, `~`, and
configuration-directory-relative paths also work. For standalone launches,
configure Proton and prefix as in `examples/standalone-proton.yaml`.

Bind defaults to read-only. `read_only: false` writes directly into the **shared
source**, including deletions. An application's updater may need writable program
files. Concurrent writable sharing follows that application's own locking rules;
GSR does not make an application multi-instance safe. An overlay can instead
provide per-game writable state over a shared read-only installation.

File mappings do not import registry entries, services, COM registrations,
runtimes, accounts or licenses from the source prefix. Those may need setup in
each game prefix. The helper runs using the game's Proton and prefix. This feature
provides the filesystem portion of shared installations; actual WeMod behavior
and trainer attachment are not yet verified. An already-running Wine service
outside this launch may have a different mount view; start with the prefix idle.

## Ordered mod merging

```yaml
version: 1
virtual_directories:
  - type: overlay
    target: '~/Games/Example/Data'
    write_dir: '~/Games/ModState/Example/default'
    layers:
      - path: '~/Games/Mods/Example/Textures'
      - path: '~/Games/Mods/Example/Experimental'
        enabled: false
      - path: '~/Games/Mods/Example/Patch'
```

The original target is the lowest-priority base. Enabled layers merge recursively,
from first (lowest) to last (highest). Matching file paths in Patch override
Textures. Disabled layers are not mounted and need not exist. Overlay paths are
case-sensitive Linux paths; GSR does not normalize mod filename capitalization.
Layer entries are directories, not archives. Symlinks retain their normal semantics.

All writes, copy-ups and deletion markers go into `write_dir/upper`; backend
scratch data goes into `write_dir/work`. Both are created on demand. The base and
mod directories are not modified through the overlay. Generated files persist
across launches and override **all** layers, including subsequently reordered
mods. Review your upper directory when changing profiles. Deletion markers and
metadata are backend-managed: do not treat the upper as an ordinary merged tree.
To start fresh, stop the launch and choose a new empty `write_dir`; GSR never
silently erases saved writes. Backup the entire write directory while stopped.

Use a dedicated write directory per game/mod profile. GSR locks it across app IDs
while mounted. `upper` and `work` must be real directories on the same filesystem;
local filesystems supporting extended attributes are recommended. They must not
be symlinks. Overlay paths cannot contain colon, comma, backslash or line breaks
because the backend's mount-option syntax uses those delimiters. Spaces and
apostrophes work. Targets cannot contain colons, which Steam uses in filesystem
export lists. Overlapping targets, source/target trees and write storage are
rejected after resolving symlinks. Root and system-directory targets are rejected.

## External manifests for compatibility tools

Use `manifest` instead of inline `layers`:

```yaml
virtual_directories:
  - type: overlay
    target: '~/Games/Example/Data'
    write_dir: '~/Games/ModState/Example/default'
    manifest: '~/Games/ModProfiles/Example/active.yaml'
```

An adapter can atomically replace `active.yaml` before starting GSR:

```yaml
version: 1
layers:
  - path: '../../Mods/Example/Textures'
  - path: '../../Mods/Example/Experimental'
    enabled: false
  - path: '../../Mods/Example/Patch'
```

Manifest paths are literal absolute paths or relative to the **manifest's own
directory**; they do not expand shell syntax, `~`, environment or profile variables.
YAML and JSON are accepted. Unknown fields, duplicate keys, unsupported versions
and more than 128 layers are rejected. Files are limited to 1 MiB. Each launch
reads a snapshot; running mounts are not reordered when a manifest changes.
Atomic replacement prevents partial reads. GSR accepts at most 64 mappings.

The format is deliberately independent of MO2/Vortex. A compatibility tool can
translate their enabled state and priority into this versioned manifest without
changing GSR. There is no automatic MO2 `modlist.txt` or Vortex database importer,
plugin load-order manager, archive invalidation, registry migration, or live reload.
See [manifest schema](layers.schema.json) and [profile schema](config.schema.json).

## Lifetime, visibility and recovery

The private view wraps the complete launch, including host-scope helpers. `host`
still means outside pressure-vessel, but inside the virtual view. Required target
paths are appended to `PRESSURE_VESSEL_FILESYSTEMS_RW`, preserving configured or
inherited exports. Read-only bind mounts retain their read-only restriction.
No network, IPC or PID namespace is intentionally added by GSR. This feature is a
filesystem view, not a security sandbox; programs can still access source paths.

FUSE runs as a foreground host service. GSR waits for the mount, rolls back prior
mounts if later setup fails, watches for service exits and stops the launch on
failure. On normal exit or a handled signal, process cleanup precedes unmounting.
After hooks run while the view is still available. No prefix-wide wineserver kill
is performed. Detached services and processes that leave managed groups retain
the lifecycle limits described in [architecture.md](architecture.md); mapping
files alone cannot fix all Windows launcher detachment behavior.

Temporary mountpoints are private directories below
`$XDG_STATE_HOME/game-super-runtime/virtual-sessions/` (default `~/.local/state`).
Saved writes live at the explicit `write_dir`. A forced kill/crash can leave stale
session directories or a disconnected FUSE mount. The service requests a parent-
death signal and bubblewrap uses `--die-with-parent`, but forced termination cannot
promise ordered cleanup. After ensuring no launch uses that exact session, inspect
it with `findmnt -T /exact/session/mount`, unmount that mount with
`fusermount3 -u /exact/session/mount`, and remove only the empty session directories.
Never recursively delete a mounted tree or remove `upper` to repair a stale mount.

Validated on this host with actual bind/FUSE mounts and SteamLinuxRuntime_sniper
using harmless probe commands. Commercial games, Windows application behavior,
Ubuntu/Flatpak/Snap policy combinations and mod-manager adapters remain manual
compatibility work.
