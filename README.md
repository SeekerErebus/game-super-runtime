# Game Super Runtime

One Linux executable. One YAML file per game. Compose Steam, Proton, native games,
overlays, scripts and companion applications into a repeatable launch.

```text
game-super-runtime 1091500 %command%     # Steam launch options
game-super-runtime my-game              # launch a configured command
game-super-runtime 1091500 --edit-gui    # graphical editor
```

**AI assistants:** start with [docs/AI_GUIDE.md](docs/AI_GUIDE.md), then the
[configuration reference](docs/reference.md). The [JSON Schema](docs/config.schema.json)
provides editor hints; the binary's validator is authoritative.

## Install

Build on Linux with Rust 1.85+ and a C linker:

```sh
cargo build --release --locked
make install PREFIX="$HOME/.local"
```

Ensure `~/.local/bin` is on Steam's PATH, or use the absolute executable path in
launch options. No root access is needed for this install. See
[packaging](docs/packaging.md) for Arch/AUR preparation. Uninstall with
`make uninstall PREFIX="$HOME/.local"`; configurations and logs remain.

The executable needs Linux system libraries. Orchestrated launches additionally
use **Bash 4.4+ and `sleep`**, available in normal Steam Linux environments. The GUI
uses your browser and optionally `xdg-open`. Rust, Node, Python, GTK, Qt and a YAML
interpreter are not runtime requirements. Enable only optional tools you have
installed, including their 32-bit components when needed.

## Start with Steam

In the game's **Properties → Launch Options**, enter:

```text
game-super-runtime 1091500 %command%
```

The first launch creates `~/.config/game-super-runtime/1091500.yaml`. With empty
defaults, the original argv, environment and output are passed through. Steam
continues selecting its runtime and Proton edition.

```sh
game-super-runtime 1091500 --edit
game-super-runtime 1091500 --edit-gui
```

The GUI has common controls and a full YAML view. It runs on a random loopback
port; **Close editor** or terminal Ctrl+C stops it. `--edit-gui --no-open` prints
its URL without opening a browser. No internet connection is required.

For example:

```yaml
version: 1
overlays:
  gamemode: true
  mangohud: true
env:
  PROTON_LOG: '1'
```

Set defaults for **future** games with `game-super-runtime --edit-defaults` or
`--edit-defaults-gui`. New profiles copy `defaults.yaml` once; existing profiles
are never silently changed when defaults change.

## Launch directly

A profile used outside Steam needs a `command`:

```yaml
version: 1
name: My native game
command: ['/home/you/Games/My Game/game', '--fullscreen']
working_dir: /home/you/Games/My Game
```

Run `game-super-runtime my-game`, or put that in a desktop shortcut's `Exec=`.
A supplied Steam/CLI command takes precedence. Steam session argv and env are
not saved as a fallback because they can change between updates and sessions.
For Steam-owned games needing Steam authentication/DRM, launch through Steam.
Share an application folder across prefixes or merge ordered mods at launch using
[virtual directories](docs/virtual-directories.md). Configure them in the GUI's
Virtual directories tab or YAML. Bind mappings need bubblewrap; mod overlays also
need fuse-overlayfs and FUSE 3. These tools remain optional for ordinary profiles.
Examples: [shared application](examples/shared-app.yaml), [mod overlay](examples/mod-overlay.yaml).

See [examples](examples/) for Proton, shared-container companions and scripts.

## Inspect and troubleshoot

```sh
game-super-runtime 1091500 --validate
game-super-runtime my-game --dry-run
game-super-runtime my-game --doctor
game-super-runtime 1091500 --logs
game-super-runtime --list
```

Validation never executes configured commands. Dry-run prints the resolved plan;
doctor additionally checks host executables. For a Steam-only profile, temporarily
use `game-super-runtime 1091500 --dry-run %command%` in launch options: Steam's
console receives the plan and the game is not launched.

Errors identify the file/task and log location. The newest ten logs per app are
kept under `~/.local/state/game-super-runtime/logs/APP-ID`. These record lifecycle
and supervision diagnostics; game stdout/stderr stay connected to Steam/terminal.
Use `PROTON_LOG: '1'` for Proton diagnostics. Dry-run output includes configured
environment values; review it before sharing.

## Support boundaries

- Known Steam runtime layouts are recognized. Unusual wrappers use explicit
  `runtime.command`; see [architecture](docs/architecture.md).
- Game-scope helpers share one container invocation and selected Proton prefix.
  Foreground process groups are supervised; deliberately detached daemons need
  application-specific lifecycle handling.
- Built-in controls cover GameMode, MangoHud (Vulkan), Gamescope, vkBasalt and
  DXVK HUD. Generic argv/env supports other overlays, OpenGL MangoHud, GPU
  selection, capture tools and future options.
- Flatpak Steam needs the binary and paths accessible **inside its sandbox**.
  Host installation alone is insufficient; no Flatpak extension is installed.
- The tool does not install Proton/overlays, bypass anti-cheat, migrate prefixes,
  or guarantee game compatibility. It never automatically deletes a prefix.
  Selecting another Proton edition can still cause Proton to upgrade its prefix.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
node tools/gui-smoke.mjs       # optional: Node 22+, Chromium
bash tools/make-dist.sh        # source archive + local Arch recipe
```

Tests use temporary directories, fake runtimes and real subprocesses, without
launching installed games. See [validation](docs/validation.md) for real runtime
checks. License: [MIT](LICENSE).
