# Current checkpoint

## Objective and status
Version 0.2 virtual directory extension is implemented, validated and packaged.
Assignment: VIRTUAL_DIRECTORIES_BRIEF.md (verbatim), BRIEF.md; decisions D-004/D-005
in DECISIONS.md. Original v0.1 assignment remains in ORIGINAL_BRIEF.md.
All authorized implementation milestones complete. No commit/push/global install
or live game/profile/prefix edits performed. No pending question or approval.

## Delivered
- src/virtual_dirs.rs: bind/overlay config, manifest v1, canonical path/overlap
  checks, optional tool/device preflight, foreground FUSE services, persistent
  upper/work locks, bwrap view, startup rollback, service monitoring and cleanup.
- src/config.rs/plan.rs/execute.rs: JSON plan, later-field PREFIX substitution,
  virtual-only cwd/executable diagnostics, Steam exports, outer wrapper placement,
  host_environment companion preset and remaining-launch-group bounded cleanup.
- assets/editor.html: Virtual directories tab, bind/overlay forms, enabled layers
  and priority controls; desktop library environment toggle for host companions.
- docs/virtual-directories.md, docs/layers.schema.json + config schema updates,
  examples/shared-app.yaml and mod-overlay.yaml, README/reference/AI/architecture/
  validation/package/man updates. Cargo version 0.2.0; optional Arch dependencies.
- tests/virtual_dirs.rs, tests/launch.rs additions; extended browser/runtime probes.

## Final evidence
- fmt, warning-free Clippy, git diff --check: passed.
- Default cargo test --locked --offline: 46 passed, 7 intentionally ignored.
- Explicit virtual_dirs --include-ignored: all 10 passed (53 distinct total tests).
  Covers actual mounts, priority, copy-up/deletion persistence, originals, binds,
  virtual cwd/exe, locks, TERM cleanup, partial rollback, killed FUSE service and
  canonical unsafe layouts. See WORK_LOG.md for exact commands/results.
- GSR_TEST_VIRTUAL=1 runtime-smoke.sh against actual installed sniper/run: passed
  after final executor changes. Game and helper both saw overlay in pressure-vessel;
  original data unchanged; mount directories cleaned.
- Browser smoke passed mapping forms, layer order/disabled-state preservation,
  invalid YAML, reload, mobile width and shutdown. target/qa/editor.png inspected.
- Release build + staged install/version passed. target/release/game-super-runtime
  and target/package-stage/usr/bin/game-super-runtime report 0.2.0.
- Source archive + SHA256SUMS/PKGBUILD/.SRCINFO regenerated in target/dist;
  makepkg --verifysource passed. JSON schemas and shell syntax checks passed.

## Limits and manual compatibility work
- Existing target directories required; initialize prefixes/create narrow targets.
  Filesystem mappings do not migrate registry/services/Windows dependencies.
- No real WeMod, commercial modded game, Ubuntu/Flatpak/Snap policy validation.
  Existing external Wine services may retain another mount view. Detached processes
  remain outside managed-group guarantees; no global Wine kill is attempted.
- Persistent upper overrides later mod-order changes. Use dedicated write_dir.
  Forced termination may leave stale session dirs/mounts; recovery documented.
- Rust 1.85 minimum/clean Arch chroot and alternative architectures remain manual
  release checks; no claim inferred from host toolchain testing.

## Workspace and next actions
All changes uncommitted on main. testing-notes/ remains original user-owned,
untracked input and was not changed. New feature sources/docs/tests are also
untracked until user-directed commit. No tool jobs remain running. Owned temporary
/tmp/gsr-runtime-probe removed after checking processes; no test mounts remain.
Ignored target/ retains build, staging, source artifacts and GUI screenshot.

For follow-up: read this checkpoint, virtual-directories.md and relevant source,
verify actual git status, and act on the user's next direction. To configure real
apps, verify actual installation/target paths and consult registry/dependency limits.
Do not automatically change live prefixes or re-run completed tests without reason.
