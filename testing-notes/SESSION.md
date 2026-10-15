# Game Super Runtime playtesting

## Scope

- User requested real-game testing, progressing from basic cases to a complex runtime; specific cases are pending.
- Act as users/playtesters of the released interface, not project developers.
- Use `/home/seekererebus/workspace/game-super-runtime/testing-notes/` for temporary testing artifacts and notes.
- Required additional implementation outside the explicitly requested custom work is a project gap. Record it rather than silently implementing a fix.
- Preserve existing game settings and back up profiles before edits. Do not infer successful real-game compatibility from validation, dry-runs, or mock tests.

## Preparation completed — 2026-10-15

Read public README, assistant guide, configuration reference, JSON schema, architecture/lifecycle documentation, validation coverage, packaging documentation, all six example profiles, and man page.

Project: `/home/seekererebus/workspace/game-super-runtime/`.
Existing release executable: `target/release/game-super-runtime`.
No executable exists at `~/.local/bin/game-super-runtime`; installation elsewhere/PATH availability has not been checked.
Initial `git status --short --branch` reported clean `main`.
No game launched and no profile, Steam setting, prefix, installation, or project file changed during preparation.

## Documented behavior to assess

- Empty profiles pass through Steam's argv/environment; Steam selects runtime and Proton.
- Defaults copy once when a profile is created. Existing profiles are independent.
- Standalone launches need a configured command. Commands are argv arrays; supplied CLI/Steam commands take precedence.
- Validate, dry-run, and doctor do not execute configured actions, but can create missing profiles. Doctor only checks host executables.
- Game-scope helpers share a recognized or explicitly configured runtime invocation. Proton companions use the game's Proton and compatdata directory (the parent of `pfx`).
- Hooks, readiness, required companion failures, normal exits, signals, and Steam Stop require observation in real sessions.
- Foreground process groups are supervised. Detached applications and existing Wine services have documented lifecycle limits.
- Overlay rendering, real Windows companions, and commercial games remain unverified by the earlier development checks.
- Lifecycle logs use XDG state storage; game stdout/stderr remain attached to Steam/terminal. Keep observed evidence separate from user-reported outcomes.

## Findings

No functional defect established during documentation familiarization. Missing default-location installation is an initial setup observation, not a defect.

## Test 1 — Being a DIK (1126320), native Linux pass-through

- User describes Ren'Py title using default Steam Linux Runtime, previously no launch options.
- Advised launch option: `/home/seekererebus/workspace/game-super-runtime/target/release/game-super-runtime 1126320 %command%`.
- User ran it and reports normal launch, barely perceptible lag, no apparent issues.
- Confirmed generated `/home/seekererebus/.config/game-super-runtime/1126320.yaml` contains only comments and `version: 1`; no defaults.yaml currently present.
- Log `/home/seekererebus/.local/state/game-super-runtime/logs/1126320/79232-1792094621507797957.log` records zero companions, shared runtime false, exit status 0.
- Persistent empty lockfile exists at `/home/seekererebus/.local/state/game-super-runtime/locks/1126320.lock`; its existence is normal and does not establish an active lock.
- Result: PASS for user-observed startup and verified profile creation/clean exit. No timing measurement or independent visual/gameplay verification performed.
- Shared runtime false means no shared-container orchestration was reported; it does not establish that Steam's supplied command ran without a Steam runtime. Lifecycle log does not capture exact inherited argv/environment.

## Next step

Bodycam Steam Stop cleanup passed. User corrected Chaos Theory observation: game started and ran successfully; shutdown hung, including independent script run. Both sessions cleaned up. Pragmata standalone passed in prepared environment. Dawnwalker usable trainer functionality deferred. Await next test/development direction.

## Test 6 — Dawnwalker delayed native companion

- User requests replacing FLiNG trainer with Kate ten seconds after launch; reports Kate was closed.
- Backed up original trainer profile to `/home/seekererebus/workspace/game-super-runtime/testing-notes/3751260-fling-original.yaml`.
- Changed existing 3751260 profile companion to `/usr/bin/kate --startanon`, host scope, optional, start_after_game true, start_delay_ms 10000. Installed Kate man page confirms startanon creates a new anonymous instance, avoiding existing-session reuse.
- Validate passed; doctor with placeholder `/usr/bin/true` passed (not actual Steam command). No game launched by assistant. `kate --help` in restricted sandbox returned 134 without output; used installed man page instead.
- Host delay is measured from runtime/main-process spawn, not rendered game readiness. Existing Steam GSR launch option remains valid. Pending user observation of delay, optional editor close, and game-exit cleanup.
- First run: user reports no Kate window despite live game. Log `96517-1792099310435995968.log` confirms Kate spawn then exit 1; game remained live. Steam console-linux.txt identifies exact loader failure: Steam pinned_libs_64/libcurl.so.4 lacks CURL_OPENSSL_4 required by system libproxy/libpxbackend-1.0.so.
- Inspected only selected library environment fields in live supervisor: LD_LIBRARY_PATH contains Steam runtime pinned libraries, SYSTEM_LD_LIBRARY_PATH exists and is empty, LD_PRELOAD contains Steam overlay libraries. Host scope leaves inherited Steam environment intact; it does not clean desktop library environment.
- Added per-Kate env overrides LD_LIBRARY_PATH `${env.SYSTEM_LD_LIBRARY_PATH}` and LD_PRELOAD empty. Game env unchanged. Validate passed. This is documented per-task environment configuration, not code modification. Requires game restart for retry; no process launched/stopped by assistant.
- Confirmed optional native companion failure does not stop game. Desktop host-helper environment guidance is a documentation/usability gap; successful Kate display/delay and cleanup still pending.
- Retry user observations: Kate launches successfully; closing Kate does not close game; closing game closes managed Kate. A separately launched start-menu Kate instance opens successfully and survives game exit while managed Kate closes.
- Result: PASS for configured delayed companion startup (no measured timing reported), optional companion exit, close-with-game lifecycle, and isolation from independently launched instance. Environment override resolved observed loader failure. Windows trainer lifecycle limitations must not be generalized to all companions; native managed instance cleanup worked.

## Test 2 — Bodycam (2406770), existing modifiers via YAML

- Original Steam launch options: `MANGOHUD=1 LOW_LATENCY_LAYER=1 game-performance %command%`.
- User requests identical functionality with GSR launch options and a profile written before launching.
- No prior profile existed. Created `/home/seekererebus/.config/game-super-runtime/2406770.yaml` with `version: 1`, `name: Bodycam`, explicit env `MANGOHUD: '1'` and `LOW_LATENCY_LAYER: '1'`, and `wrappers: [[game-performance]]`.
- Confirmed `game-performance` resolves to `/usr/bin/game-performance` in this session.
- `game-super-runtime 2406770 --validate`: passed.
- `game-super-runtime 2406770 --dry-run -- /usr/bin/true`: showed exactly those two env additions, one game-performance wrapper, no hooks/companions/runtime override, no warnings. This was a planning check with a placeholder command; nothing launched and actual Steam argv was not inspected.
- Replacement Steam launch options: `/home/seekererebus/workspace/game-super-runtime/target/release/game-super-runtime 2406770 %command%`.
- Status: prepared; actual startup, MangoHud, low-latency behavior, and game-performance functionality await user observation. No implementation changes required so far.
- Live verification: user reports game running and looking good. Host process inspection confirms GSR PID 80821 → systemd-inhibit PID 80822 → powerprofilesctl launch PID 80831 → Steam runtime/Proton → Bodycam-Win64-Shipping.exe PID 81078.
- `powerprofilesctl get` reports `performance`; `powerprofilesctl list-holds` shows a performance hold with reason `Launched with CachyOS game-performance utility`, tied to Steam's launch wrapper. `systemd-inhibit --list` confirms the matching shutdown/sleep/idle blocker.
- game-performance is a wrapper script that execs these tools, not a persistent process/service of its own. Its intended power-profile hold and inhibitor are confirmed active. No independent verification of low-latency layer rendering/effect or post-exit hold release yet.
- Steam Stop test: user launched Bodycam again, then used Steam Stop. Latest log `94585-1792098719097511329.log` records start with zero companions/shared runtime false and exit status 1, with no signal-forwarding or additional diagnostic lines. Status 1 alone does not explain which layer returned it or establish a GSR defect.
- Post-stop host inspection: no matching Bodycam, GSR app 2406770, SteamLaunch app 2406770, wineserver, powerprofilesctl launch or CachyOS systemd-inhibit process. `powerprofilesctl list-holds` empty; current profile balanced; inhibitor list contains no game-performance entry. Result: PASS for observed Steam Stop cleanup in this case.

## Test 3 — The Blood of Dawnwalker (3751260), Proton trainer companion

- User reports no current launch options, requests trainer `/home/seekererebus/Games/Trainers/The Blood of Dawnwalker Trainer - FLiNG.exe`; file existence confirmed.
- User's desired functionality: concurrent applications, optional delays, selectable close-with-game versus independent lifetime, and optional helpers closing without killing the game.
- Created `/home/seekererebus/.config/game-super-runtime/3751260.yaml`; no previous profile existed. Single game-scope Proton companion, trainer directory as cwd, `required: false`, `start_after_game: true`, `start_delay_ms: 0`. No Proton/runtime override; Steam supplies them.
- `--validate` passed. Actual Steam command, runtime recognition, prefix sharing, trainer startup/attachment, and lifecycle remain unverified until launch.
- Launch options: `/home/seekererebus/workspace/game-super-runtime/target/release/game-super-runtime 3751260 %command%`.
- Expected: game and trainer start close together (sequential spawn, not guaranteed simultaneous readiness); optional trainer exit should leave game running; managed trainer processes should be cleaned up when game exits. Wine delegation/detachment may limit cleanup, as documented.
- Documented capability gap: no per-companion setting to remain open independently after the main game exits. Recording this gap rather than adding implementation or a detachment workaround.
- User observation: game launched normally. Companion opened its setup/launcher, requested WeMod installation, and lacked network access. User deferred WeMod testing pending separately planned proper virtual-drive support. No cause of network failure established; do not attribute it to GSR without evidence.
- User observation: closing the game did not close the side launcher; Steam continued showing the title as running. User accepts this behavior as a default but wants a future close-all-companions policy.
- User explicitly confirms reciprocal test too: closing the optional trainer app did not close the game. Optional companion exit behavior therefore passed; previous summary incorrectly listed it as untested.
- Both available lifecycle logs report one companion, shared runtime true, automatic Proton `run` selection to avoid companion wineserver waits, companion start, and GSR exit status 0. Logs do not establish the side launcher's actual exit or identify why it survived.
- Result: game plus companion setup startup observed; usable trainer/attachment untested. Automatic close-with-game was not achieved in this case. Independent survival here is an observed lifecycle limitation, not proof of a supported selectable lifetime policy. Shared-prefix identity was not independently verified.

## Test 4 — Pragmata, standalone execution

- User requests `game-super-runtime Pragmata`, without Faugus open, matching executable, existing prefix, Proton-CachyOS Latest and game-performance.
- Read current Faugus games.json entry: matches supplied paths, no game arguments, wrapper game-performance, no enabled additional apps. Faugus uses UMU by default; this test instead uses GSR's documented direct Proton/runtime configuration.
- Confirmed executable exists; existing prefix `/home/seekererebus/Games/Prefixes/Pragmata/pfx` is symlink `.`. No prefix changes made.
- Selected exact installed Proton at `~/.local/share/Steam/compatibilitytools.d/Proton-CachyOS Latest/proton`; manifest requires runtime app 4183110 (installed SteamLinuxRuntime_4). Configured its explicit `_v2-entry-point --verb=run --` wrapper.
- Created `~/.config/game-super-runtime/Pragmata.yaml` with game executable, game directory cwd, game-performance wrapper, existing Proton/prefix/Steam root and standalone Steam IDs 0. No existing Pragmata profile was present.
- Validate and doctor passed. Actual game launch and compatibility not tested by assistant; user will run it.
- GSR is still not installed in ~/.local/bin or found on this shell PATH. User can add the existing release directory to terminal PATH for the requested command spelling.
- First launch stalled without usable window. User output reached protonfixes and ntsync; host inspection confirmed GSR, runtime, Proton, Windows steam.exe and PRAGMATA.exe remained running. Power-profile hold active. GSR log only recorded launch.
- Installed Proton code inspection reveals UMU_ID selects umu.exe for non-Steam startup; without it the direct profile selected steam.exe. Installed UMU defaults GAMEID/UMU_ID to umu-default when no fix ID configured. This is a plausible context mismatch, not proven root cause.
- Backed up first profile to `/home/seekererebus/workspace/game-super-runtime/testing-notes/Pragmata-first-attempt.yaml`. Added `UMU_ID: umu-default` and explicit STEAM_COMPAT_INSTALL_PATH to game directory. Validate and doctor passed again; retry pending. Did not stop running game or change GSR code. First attempt is not a successful standalone test; documentation lacks this build-specific non-Steam context guidance.
- Before retry, user launched via Faugus to confirm viability; reports it updated Proton and internal elements including BattlEye/EAC. Exact updated components/versions not independently captured. This intervening environmental change prevents attributing success solely to the YAML adjustment.
- User reports subsequent GSR launch successfully started game, then closed it. Result: PASS for standalone startup in the current prepared environment. Wrapper activity and cleanup were not independently checked for this successful run; user assumed wrapper worked.
- Logs confirm first run received SIGINT and returned 130; second run returned 0.
- Additional interruption test: user launched Pragmata and sent SIGINT during startup loading screens with game window already open. Latest log `102849-1792099772929701103.log` confirms signal 2 forwarded to launch process group and exit 130.
- Post-interruption host inspection found no actual matching Pragmata/GSR, wineserver, powerprofilesctl launch or game-performance inhibitor process (search itself matched and was excluded). Power profile balanced, no profile holds, no game-performance inhibitor. Result: PASS for observed standalone startup SIGINT cleanup. Log does not expose precise propagation through container or individual cleanup steps.
- Provisioning/update scope: public README explicitly excludes installing Proton/overlays; reference excludes Proton download/runtime auto-selection. Faugus dependency/update handling has not been demonstrated by GSR and should not be inferred from this success. Record as potential future product scope, not an existing promised feature.

## Test 5 — Splinter Cell Chaos Theory, custom Bash package

- User confirms existing `~/Games/Splinter.Cell-jc141/chaos.theory.sh` package runs on this system; requests standalone GSR launch with ID `Splinter-Cell-Chaos-Theory`.
- Read entry script: Bash, changes cwd to its own directory, sources actions.sh, manages dwarfs mount/extraction, its own Wine prefix/settings, optional isolation and cleanup traps, and launches game.
- Created previously absent `~/.config/game-super-runtime/Splinter-Cell-Chaos-Theory.yaml`: command array `/bin/bash` plus absolute expanded script path; working directory is package directory. No GSR Proton/runtime overrides or extra hooks needed; package handles its own internals.
- Validate, doctor and dry-run passed. Doctor does not verify package-internal tools or successful mounts/isolation. No game or script executed by assistant, no package modifications or new custom code.
- User launch command: `game-super-runtime Splinter-Cell-Chaos-Theory`. Actual startup/exit and package cleanup await observation.
- User reports hang, suspects Gamescope, requests runtime cleanup. Host tree showed GSR 92545, Bash 92546, Gamescope 92577, gamescopereaper 92608, plus detached dwarfs 92565 and fuse-overlayfs 92573. No splintercell3.exe observed. This does not prove Gamescope is the root cause.
- Sent TERM to GSR 92545. Log confirms signal forwarding and exit 143. Bash/Gamescope and both mount services exited, and both package mounts disappeared. gamescopereaper 92608 remained in launch PGID 92546, ignored TERM, required targeted KILL. No global Wine kill, prefix deletion, or package/config changes performed.
- Result: launch unsuccessful; cleanup required manual removal of surviving gamescopereaper. Record as supervision cleanup gap for this package. Final verification checked original PIDs and package mount paths.
- Control test: user launched script directly without GSR and observed the same hang. This points to package/environment failure rather than GSR as cause of startup hang; precise cause still unknown. GSR cleanup limitation remains separately observed.
- User requested standalone cleanup. Identified script session/process group 93375, Gamescope 93405, reaper 93440, dwarfs 93393 and overlayfs 93402. TERM to group cleared script/Gamescope/mounts; reaper again survived and required targeted KILL. Final check confirmed all identified PIDs and mounts gone. No package or profile edits.
- CORRECTION superseding prior startup-failure interpretations: user clarifies Chaos Theory started and ran fine; it failed to close properly. Result is PASS for custom Bash package game startup, FAIL for clean shutdown. Independent script run also exhibited shutdown hang. Exact root cause remains unestablished; manual cleanup evidence above remains valid.
- User proposes deferring more involved companion usability tests until planned virtual-directory work: shared companion installation exposed across prefixes and runtime mod overlays without replacing normal game files. No implementation authorized by this discussion alone.

## Handoff location

User requested moving all testing artifacts into this testing-notes directory and removing the temporary ~/gsr-notetaking directory. Moved the entire directory, including both backup YAML files, and updated absolute note references. Active profiles remain unchanged in ~/.config/game-super-runtime/. No implementation changes, commits, or installation performed.
