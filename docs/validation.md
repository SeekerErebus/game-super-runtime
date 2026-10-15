# Validation and remaining coverage

Automated checks run in the development workspace on x86_64 Linux:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --release --locked --offline
node tools/gui-smoke.mjs
bash tools/runtime-smoke.sh /path/to/SteamLinuxRuntime_sniper/run
make install DESTDIR="$PWD/target/stage" PREFIX=/usr
bash tools/make-dist.sh
```

The Rust suite covers literal argv (including non-UTF-8), defaults, storage,
strict schema errors, variable cycles, wrapper order, recognized Steam chains,
Proton overrides and companion prefix matching, hooks, signals, readiness,
delayed helpers, nested cleanup, launch locks and local-editor HTTP behavior.
It uses fake executables and real child processes; it does not launch a game.

The browser check uses Node 22+ and Chromium through the Chrome DevTools Protocol,
without npm dependencies. It verifies form saving, raw YAML rejection, reload,
mobile fit and server shutdown. It writes `target/qa/editor.png` for visual review.
The profile and browser user data are temporary. Node/Chromium are development
tools only. Restricted environments may need permission to bind loopback sockets.

The explicit runtime smoke test ran successfully against the locally installed
SteamLinuxRuntime_sniper (Bash 5.1.4). It verifies that the game probe and companion
have the same mount namespace, distinct from the host. It uses temporary configs
and harmless Bash commands. Runtime state defaults to `/tmp/gsr-runtime-probe`;
set `GSR_RUNTIME_TEST_STATE` to use another scratch directory. Steam may copy
about 2 GiB there if hard links across filesystems are unavailable. Remove the
scratch runtime after testing when no probe is running. The normal Steam runtime
requires its own lock file even when scratch state is separate.

No commercial game, Windows Cheat Engine installation, GPU overlay rendering,
Steam Deck, Flatpak Steam, aarch64 system, or all Proton/runtime combinations
has been verified in this workspace. Those are the release/manual test matrix,
not claims inferred from a passing mock runtime. Prefix service behavior and
daemonized processes are limited as described in [architecture.md](architecture.md).

Before publishing a release, test at least a native game, a Steam Proton game,
a foreground Windows companion in the same prefix, an overlay on Vulkan and
OpenGL, and Steam's Stop button on each intended runtime version. Verify the
Arch package in a clean chroot and build on the claimed minimum Rust version.

## Version 0.2 virtual directory coverage

Run the default tests without mount dependencies, then explicitly opt into the
real namespace/FUSE tests on a host with bubblewrap, fuse-overlayfs and FUSE 3:

```sh
cargo test --locked --offline
cargo test --locked --offline --test virtual_dirs -- --include-ignored
GSR_TEST_VIRTUAL=1 bash tools/runtime-smoke.sh /path/to/SteamLinuxRuntime_sniper/run
```

Validated locally: 46 default tests passed; all 10 virtual-directory tests passed
(including the 7 normally ignored host-dependent tests), for 53 distinct passing
tests. Actual mounts verify low-to-high precedence, recursive merge, copy-up,
persistent deletions and writes, original preservation, read-only/writable binds,
virtual-only executables/cwd, cross-app write locking, SIGTERM cleanup, rollback
when a later mount fails, and launch shutdown after killing a FUSE service. Unsafe
or aliased overlapping layouts and absent targets fail without starting the game.
The no-dependency test launches a plain profile with an empty PATH and verifies
that a bind-enabled profile instead reports the missing tool.

The SteamLinuxRuntime_sniper smoke with GSR_TEST_VIRTUAL=1 passed on actual
pressure-vessel: both probe and companion see the overlay; the original target
retains its base file. This uses temporary data, not an installed game or Windows
application. The new remaining-group cleanup and host desktop-library preset
have process regression tests based on the playtesting findings.

The browser smoke now covers virtual mapping form save/reload, layer enable/order
preservation, invalid YAML rejection and mobile width. The directory tab screenshot
was visually inspected. Fmt, warning-free Clippy, release build and packaging are
required with every release. No new real-game, Windows application portability,
Ubuntu policy, or Flatpak/Snap claim is inferred from these passing checks.
