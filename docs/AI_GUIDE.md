# Operational contract for assistants

Read this before generating or changing a profile. Use [reference.md](reference.md)
for exact keys and [examples](../examples/) for complete configurations.

## Establish context

1. Determine app ID, Steam vs standalone launch, and actual paths to the game,
   Proton and optional tools. Do not guess a library path.
2. Locate the profile with `game-super-runtime APP-ID --config-path`. Precedence:
   `--config-dir`, then `GSR_CONFIG_DIR`, then XDG config home.
3. Read existing YAML and preserve unrelated settings. Defaults are copied only
   on creation, never merged into existing profiles.
4. Steam-only profiles can leave `command` empty. Standalone launches need it.
5. Back up before editing, or use the GUI's validated atomic save and backup.

## Generate precise configurations

- Version is `1`. Unknown fields, duplicate keys and malformed YAML are errors.
- Commands are argv arrays. Never combine executable and arguments into a shell
  string. Never put shell quote characters around a path inside an argv word.
- `wrappers` surround the complete launch, first entry outermost. `game_wrappers`
  surround only the game command inside the runtime. Include `--` when required
  by a wrapper's own CLI. Wrapper programs must execute the remaining arguments.
- `env` applies globally. `proton.env` overrides it; per-task `env` overrides both.
  Overlay convenience settings only fill keys not explicitly set. Quote values.
- Expansion: `${APP_ID}`, `${HOME}`, `${CONFIG_DIR}`, `${vars.NAME}`, `${env.NAME}`.
  Environment substitution reads inherited values, not other YAML env entries.
  Unknown variables/cycles fail. `$$` means a literal dollar in expanded fields.
- Script bodies are literal Bash, with no GSR substitution. Use `GSR_APP_ID`,
  `GSR_CONFIG_DIR`, `GSR_VAR_NAME`, and optional `GSR_PROTON`/`GSR_PREFIX` there.
  After hooks also receive `GSR_EXIT_CODE`. Arguments become `$1`, `$2`, etc.
- Relative path fields resolve against the config directory; relative argv is
  relative to process cwd. Prefer absolute executable paths. `~` expands only in
  path fields; use `${HOME}` in argv.

## Shared containers and prefixes

`scope: game` (default) places helpers inside the same runtime invocation.
`proton: true` on a companion uses the game's exact Proton and compatdata path.
`proton.prefix` is the directory **containing** `pfx`, never `pfx` itself.
Do not override the prefix in a Proton companion's environment.

With Proton companions, the inferred game verb changes from `waitforexitandrun`
to `run`, avoiding a wait for the helper's wineserver. Explicit conflicting wait
verbs are rejected. Proton's own prefix initialization and locking still apply.

`scope: host` runs outside the supplied Steam runtime. Without a runtime, scopes
collapse onto the host. Use `runtime.mode: required` when a container is essential.
`runtime.command` explicitly supplies a container wrapper prefix; never combine
it with an incoming Steam command that already has a recognized runtime.

Companions must remain in the foreground. `required: true` treats any exit as a
failure while the game runs. Readiness uses a file the helper creates; clear
stale markers in a before hook. `start_after_game: true` plus `start_delay_ms`
supports tools that attach after launch.

## Verify before running

```sh
game-super-runtime APP-ID --validate
game-super-runtime APP-ID --dry-run -- /actual/Steam/command args
game-super-runtime APP-ID --doctor -- /actual/Steam/command args
```

These checks create a missing profile from defaults but never execute configured
actions, create prefixes or acquire launch locks. Existing profiles are unchanged.
Dry-run output can contain private env values; review before sharing it.
Doctor checks host executables, not GPU behavior or all container-only files.
Test complex flow with harmless fake programs and a temporary config directory.
Never describe a successful mock test as verified game/GPU compatibility.

Exit `125` is used for launcher/supervision errors, `124` for hook timeouts, and
`128 + signal` for interruption. Other child statuses are preserved (games can
also return 124/125; consult logs). Read the printed log and original child stderr.
Do not recommend prefix deletion, Steam env removal, runtime removal, Proton
replacement, anti-cheat bypass or global wineserver kills as generic fixes.
Configuration scripts are user code and must be reviewed like shell scripts.

## Virtual directory profiles

Read [virtual-directories.md](virtual-directories.md) before configuring shared
prefix folders or mods. Use existing, narrow directory targets; never migrate or
replace entire prefixes. Bind defaults to read-only; writable binds update the
shared source. Overlays merge enabled layers in low-to-high order and preserve
writes separately. Use a dedicated write_dir. Export external manifest v1 for
future mod-manager adapters; its paths are literal and relative to the manifest.
No automatic registry or MO2/Vortex integration is implied. Plain YAML keeps its
original dependency footprint. Doctor checks tools/paths but does not test mounts.

Use `host_environment: true` on host desktop companions that need system library
paths instead of Steam's pinned libraries. This is opt-in and does not change the
game. For non-Steam Proton variants that require UMU context, inspect the installed
build's requirements: the playtest used `UMU_ID: umu-default` and explicit
`STEAM_COMPAT_INSTALL_PATH`. An intervening Faugus update prevents attributing its
successful launch solely to those values. GSR does not provision or update UMU,
Proton or runtime dependencies, and does not inject those build-specific settings.
