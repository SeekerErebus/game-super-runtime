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
