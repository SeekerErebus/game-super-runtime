# Configuration reference — version 1

Empty files and `version: 1` are valid pass-through profiles. Maximum size: 1 MiB.
Unknown fields, duplicate keys and malformed YAML fail validation. YAML value
anchors are supported; merge keys (`<<`) are not. Quote environment values.

## Storage and command line

| Item | Location / behavior |
| --- | --- |
| Profile | `CONFIG/APP-ID.yaml` |
| CONFIG | `--config-dir`, then `$GSR_CONFIG_DIR`, then `$XDG_CONFIG_HOME/game-super-runtime` |
| Config fallback | `~/.config/game-super-runtime` |
| Defaults | `CONFIG/defaults.yaml`, copied once for each new profile |
| STATE | `$XDG_STATE_HOME/game-super-runtime`, fallback `~/.local/state/game-super-runtime` |
| Logs | `STATE/logs/APP-ID/`, newest ten launch logs |
| Locks | `STATE/locks/APP-ID.lock` prevents simultaneous launches of one ID |
| GUI backup | `APP-ID.yaml.bak` contains the previous text |

`--config-dir` may be relative to cwd. XDG variables must be absolute. App IDs:
1–128 ASCII letters, digits, dots, underscores or hyphens; no leading dot;
`defaults` is reserved. Flags precede the game command. After the executable,
all words belong to the game. `--` explicitly ends GSR option parsing.

`--edit` uses `VISUAL`, then `EDITOR`, then desktop `xdg-open` or a system editor.
Editor commands may contain quoted arguments but are not interpreted as shell
programs. Use your editor's wait option if needed. Both editors create missing
profiles and allow malformed existing files to be repaired.

## Root fields

| Field | Type / default | Meaning |
| --- | --- | --- |
| `version` | integer `1` | Schema version |
| `name` | string or null | Display label |
| `command` | argv array `[]` | Fallback when no Steam/CLI command is supplied |
| `working_dir` | path or null | Default process cwd; otherwise inherit |
| `variables` | string map `{}` | Substitution values, also exported as `GSR_VAR_NAME` |
| `env` | string map `{}` | Set values, preserving other inherited environment |
| `unset_env` | array `[]` | Remove inherited keys; cannot conflict with added settings |
| `wrappers` | array of argv arrays | Wrap full launch, first entry outermost |
| `game_wrappers` | array of argv arrays | Wrap only game command inside runtime |
| `overlays` | object | Convenience controls below |
| `proton` | object or null | Proton overrides below |
| `runtime` | object | Container controls below |
| `scripts` | map of objects | Named explicit Bash programs |
| `before`, `after` | action arrays | Ordered blocking hooks |
| `companions` | companion array | Managed concurrent programs |
| `virtual_directories` | mapping array | Private shared folders and ordered mod overlays |
| `shutdown_timeout_ms` | integer `3000` | TERM-to-KILL grace, 0–60000 milliseconds |

Names in env/variables match `[A-Za-z_][A-Za-z0-9_]*`; `GSR_` is reserved. Empty
arguments are allowed; empty executables are not. Commands never undergo shell
word splitting, globbing or command substitution.

### Expansion and paths

Configured argv, env, variables and paths support `${APP_ID}`, `${HOME}`,
`${CONFIG_DIR}`, `${vars.NAME}` and `${env.NAME}`. Environment references read
inherited values, not another YAML env entry. Variables can refer to each other;
unknown values and cycles fail. `$$` escapes a literal dollar. Supplied Steam/CLI
argv and script bodies are not expanded.

Path fields (`working_dir`, Proton paths, script paths, ready files, vkBasalt
config) expand `~`/`~/` and resolve relative to CONFIG. Argv words do not expand
`~`; use `${HOME}`. Relative argv paths are relative to process cwd.

## Overlays

| Key | Default | Effect |
| --- | --- | --- |
| `gamemode` | false | Add `gamemoderun` |
| `mangohud` | false | Set `MANGOHUD=1` (Vulkan) |
| `mangohud_config` | null | Set `MANGOHUD_CONFIG` |
| `gamescope` | null | Enable with argv, e.g. `['-W', '1920', '-f']`; `[]` enables defaults |
| `vkbasalt` | false | Set `ENABLE_VKBASALT=1` |
| `vkbasalt_config` | null | Set `VKBASALT_CONFIG_FILE` |
| `dxvk_hud` | null | Set `DXVK_HUD`, e.g. `fps,devinfo` |

False/null adds nothing; inherited settings survive. Explicit env overrides these
conveniences. Nesting: user wrappers → Gamescope → GameMode → host supervisor →
runtime → game supervisor → game_wrappers → Proton/game. Global overlay env can
affect helpers; per-task env can disable it. OpenGL MangoHud can use
`wrappers: [[mangohud, --dlsym]]`. Other layers/overlays, Mesa HUD, vkCapture, PRIME,
GameMode options and custom tools use generic env/wrappers. Steam overlay env is
preserved. Install the required Vulkan layers and 32-bit libraries separately.

## Proton

| Key | Default | Meaning |
| --- | --- | --- |
| `path` | null | Proton directory or executable; replaces identified Steam Proton |
| `prefix` | null | Compatdata directory **containing** `pfx` |
| `steam_root` | null | `STEAM_COMPAT_CLIENT_INSTALL_PATH`; inherit or detect `~/.steam/root` |
| `verb` | null | `run`, `waitforexitandrun`, `runinprefix` |
| `env` | `{}` | Arbitrary controls overriding root env |

Any installed Proton edition can be selected by path. The runtime is still
selected by Steam, unless explicitly configured separately. Choose a compatible
runtime for your Proton edition. There is no download or runtime auto-selection.
New explicit Proton commands default to `waitforexitandrun`; existing verbs are
preserved. Proton companions change an inferred wait verb to `run`; an explicitly
conflicting wait verb is rejected because it would wait for the companion to exit.

Explicit Proton settings require a compatdata path. GSR creates a configured
directory only at launch; Proton initializes/upgrades its contents. `WINEPREFIX`
alone does not override Proton's prefix. Numeric IDs fill missing `SteamAppId`,
`SteamGameId` and `STEAM_COMPAT_APP_ID` with explicit Proton settings. Other IDs
leave that context to Steam/user. Empty pass-through profiles inject no IDs.

## Runtime

`runtime.mode`: `auto` (default), `required`, or `none`. Auto recognizes known
Steam container prefixes: `_v2-entry-point … --`, `pressure-vessel-wrap … --`,
SteamLinuxRuntime `run … --`, and `steam-runtime-launcher-interface-0
container-runtime`, optionally behind `reaper` / `steam-launch-wrapper`.
Required errors when no boundary exists. None treats the command as opaque and
rejects game-scope features when a known container would otherwise be hidden.

Explicit `runtime.command` is an argv prefix accepting the inner command:

```yaml
runtime:
  mode: required
  command:
    - '${HOME}/.steam/root/steamapps/common/SteamLinuxRuntime_sniper/_v2-entry-point'
    - --verb=run
    - --
```

Do not combine it with an incoming Steam command already containing a runtime.
A custom wrapper is allowed; the same-container guarantee then depends on that
wrapper actually executing the remaining arguments together.

## Scripts and hooks

```yaml
variables:
  TARGET: '${CONFIG_DIR}/session.ready'
scripts:
  prepare:
    body: |
      printf '%s\n' "$GSR_APP_ID" > "$GSR_VAR_TARGET"
  cleanup:
    path: scripts/cleanup.sh
before:
  - script: prepare
    scope: host
after:
  - script: cleanup
    args: ['${vars.TARGET}']
    scope: host
    allow_failure: true
```

Scripts contain exactly one `body` or `path`; Bash uses `-euo pipefail` without
profile/rc files. Args become `$1`, `$2`, etc. Env changes in a script do not
propagate back to the supervisor. Use root env for persistent launch settings.

| Action key | Default | Meaning |
| --- | --- | --- |
| `name` | generated | Diagnostic label |
| `command` / `script` | exactly one | argv or named script |
| `args` | `[]` | Arguments for a named script only |
| `env` | `{}` | Per-task overrides |
| `working_dir` | null | Per-task cwd |
| `scope` | `game` | `host` outside runtime; `game` inside |
| `allow_failure` | false | Continue on nonzero status |
| `timeout_ms` | null → 30000 | 1–86400000 milliseconds |

Host before hooks/companions precede the runtime and its game-scope hooks/helpers.
Without a runtime all scopes collapse and YAML order is used. After hooks run in
listed order after child cleanup, including after before-hook failures/signals
once that supervisor starts. Inner hooks cannot run if the runtime never starts.
All after hooks run even if one fails. `GSR_EXIT_CODE` contains the original result;
after failure only changes a successful result. GSR target variables are described
in [AI_GUIDE.md](AI_GUIDE.md).

## Companions

Use required unique `name`, `command` or `script`, plus optional `args`, `env`,
`working_dir`, `scope`. Additional keys:

| Key | Default | Meaning |
| --- | --- | --- |
| `proton` | false | Use game's Proton/prefix; game scope and command only |
| `required` | false | Stop launch when helper exits (even status 0) |
| `start_after_game` | false | Spawn after this scope's main process starts |
| `start_delay_ms` | 0 | Delay before spawn, max 3600000, helpers processed in order |
| `ready_file` | null | Wait for file existence before proceeding |
| `ready_timeout_ms` | 10000 | Readiness deadline, 1–3600000 |

Optional helper failures log and continue. Readiness timeout stops the helper;
required helpers also stop the launch. After-game delays do not postpone game
startup. For host scope, the runtime process can start before the game itself.
Clear stale ready files in a before hook. GSR never deletes arbitrary markers.
Keep companions in the foreground; detached daemons need explicit lifecycle code.

## Upstream references

Integration names were checked against [Proton](https://github.com/ValveSoftware/Proton),
[Steam Runtime](https://github.com/ValveSoftware/steam-runtime),
[MangoHud](https://github.com/flightlessmango/MangoHud),
[GameMode](https://github.com/FeralInteractive/gamemode),
[Gamescope](https://github.com/ValveSoftware/gamescope), and
[vkBasalt](https://github.com/DadSchoorse/vkBasalt). Generic env/argv supports new
options as those projects evolve.

## Virtual directories

`virtual_directories` is an optional array (default `[]`, at most 64). Each entry
has `type: bind`, required `source`/`target`, and `read_only` (default true); or
`type: overlay`, required `target`/`write_dir`, and either `layers` (default `[]`)
or `manifest`. Layers contain required `path` and optional `enabled` (default true).
Later enabled layers win; persistent writes override all layers. Sources and
targets must exist. `${PREFIX}` is available here after Proton selection.

See [virtual directories](virtual-directories.md) for dependency setup, the external
manifest contract, complete examples, persistence, lifecycle and limitations.

For native host companions, `host_environment: true` restores `LD_LIBRARY_PATH`
from the launcher's inherited `SYSTEM_LD_LIBRARY_PATH` (empty if absent) and clears
`LD_PRELOAD`. It requires `scope: host`; default false preserves old behavior.
Explicit companion `env` entries override this preset. This fixes desktop programs
such as Kate failing against Steam's pinned libraries without changing game env.
