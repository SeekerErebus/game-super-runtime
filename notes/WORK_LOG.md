# Work log

Record milestones, useful investigations, failed approaches, and handoffs.
Append dated entries in chronological order. Keep the next action in `STATE.md`;
this file is supporting history, not the current task list. Link detailed logs
or topic notes when an entry would otherwise become large.

## 2026-10-15 — Workspace prepared

- Request: Establish a basic workflow for a large AI development project that
  may span repeated context compactions.
- Initial evidence: Empty project workspace; Git repository has no commits.
- Changes: Added agent instructions, README, and four continuity notes.
- Verification: Documentation-only setup; no application tests apply.
- Handoff: Await the detailed project prompt, preserve its requirements, and
  establish milestones. No application stack was selected.

## Future entry format

### YYYY-MM-DD — Milestone or investigation

- Outcome and affected files:
- Evidence / validation: Exact command or observation, result, and limits.
- Unsuccessful approaches worth remembering:
- Follow-up or relevant decision IDs:

## 2026-10-15 — Launcher, editors, integrations and packaging implemented

- Captured the complete original assignment in ORIGINAL_BRIEF.md. Selected Rust
  with six direct crates and no installed GUI/YAML runtime dependency.
- Implemented strict typed YAML, safe defaults/storage, argv-based plans, known
  Steam runtime boundaries, Proton path/prefix/env control, common overlays and
  extensible wrappers, scripts/hooks, managed foreground companions, timeouts,
  readiness, signals, cleanup and per-app lifecycle logs.
- The in-container Bash supervisor avoids host glibc ABI mismatch. Confirmed in
  Valve Proton source that waitforexitandrun waits for wineserver; companion plans
  use run. Preserve opaque commands for pass-through without speculative parsing.
- Added browser GUI with forms/full YAML, validation, backup, conflict checking,
  loopback token and origin checks, plus standard text editor integration.
- Added AI_GUIDE/reference/schema/architecture/examples, install/uninstall rules,
  man page, Bash completion, CI and checksummed local/remote-source Arch recipe generator.
- Validation: cargo check and clippy passed; Rust tests initially 28, then 38,
  then 40 passing. Final suite adds empty script argument coverage (41 total).
- `node tools/gui-smoke.mjs`: passed form save, invalid YAML, reload, mobile fit,
  clean close. Screenshot target/qa/editor.png visually inspected.
- Real Steam sniper runtime: Bash 5.1.4 probe passed. Full runtime-smoke.sh passed
  and proved game/helper mount namespaces equal and distinct from host. No game,
  real Windows helper or GPU overlay was launched.
- `cargo build --release --locked --offline`: passed. Binary approximately 1.4 MiB;
  ldd shows only libc/libgcc_s/loader. Staged make install and --version passed.
- `bash tools/make-dist.sh` and local `makepkg --verifysource`: passed, with actual
  SHA-256 and generated .SRCINFO. No publication/commit/installation into home.

### Investigation notes / avoid repeating

- Sandbox DNS blocked the first Cargo fetch; user approved cargo prefix. Offline
  builds now work. Browser socket tests need escalation outside sandbox, approved
  node tools/gui-smoke.mjs. Cargo-launched integration tests inherit Cargo approval.
- Steam runtime opens its installed .ref file, blocked in sandbox; approved
  bash tools/runtime-smoke.sh uses temporary runtime state. /tmp is another device,
  so the initial probe copied about 1.8 GiB and emitted many hard-link warnings.
  Later runtime checks reuse the scratch copy. Clean it after final checks.
- First browser harness accidentally attached to Chromium's background extension,
  causing ERR_ABORTED with empty body. Fixed by selecting type=page, about:blank.
  This was a harness failure, not a GUI defect.
- Nested host/runtime shutdown originally allowed too little inner-hook time.
  Fixed grace calculation and covered by signal regression test. Optional missing
  hooks now honor allow_failure; empty script args remain legal.
- Git starts clean on main, setup already committed. All implementation is dirty;
  user did not request commit or push. No pending questions or decisions.

### Final acceptance evidence

- Final suite: 41 tests passed; fmt and clippy with warnings denied passed.
- Final optimized build and browser smoke against that release binary passed.
- Final staged install preserves repository-relative docs links.
- Final archive/PKGBUILD regenerated; makepkg --verifysource passed.
- Shell syntax, schema JSON and git whitespace checks passed.
- The 1.8 GiB /tmp/gsr-runtime-probe test copy was removed; no test services remain.
- Release binary, source tarball, local Arch recipe and GUI screenshot remain in
  target/. STATE.md records manual coverage limits and publication prerequisites.

## 2026-10-15 — Version 0.2 virtual directories

Preserved the new assignment verbatim in VIRTUAL_DIRECTORIES_BRIEF.md, updated
BRIEF.md and recorded D-004/D-005. Read all testing-notes evidence without editing
those user artifacts. Implemented optional bwrap binds + fuse-overlayfs merges,
manifest v1, persistent write storage/locks, lifecycle/preflight/Steam exports,
GUI controls, examples, schemas and user/developer documentation. Version 0.2.0.
Also implemented the Kate desktop-library preset and same-group reaper cleanup
observed in playtesting. No real game/profile/prefix edited; no commit or push.

Evidence:
- cargo fmt --all -- --check; cargo clippy --all-targets --locked --offline -- -D warnings: passed.
- cargo test --locked --offline: 46 passed, 7 host-dependent tests ignored.
- cargo test --locked --offline --test virtual_dirs -- --include-ignored: all 10
  passed, including those 7. Total 53 distinct passing tests. Earlier aggregate
  --include-ignored run passed all 51 then-existing tests; two added failure/safety
  cases passed in the final virtual test run.
- Actual FUSE/bwrap checks: precedence, recursive merge, persistent copy-up/deletion,
  original preservation, read-only/shared-writable mappings, virtual executable/cwd,
  duplicate write-dir lock, TERM cleanup, partial mount rollback, FUSE process
  death causing launch shutdown, canonical alias overlaps/absent targets rejected.
- GSR_TEST_VIRTUAL=1 bash tools/runtime-smoke.sh installed SteamLinuxRuntime_sniper/run:
  passed again after final executor changes. Game/helper share pressure-vessel view
  and see overlay data; base unchanged, session mount directories cleaned.
- node tools/gui-smoke.mjs: passed form saves, virtual mappings, enable/reorder
  preservation, bad YAML rejection, reload, mobile width, close; no JS exceptions.
  Visually inspected target/qa/editor.png (Virtual directories tab).
- cargo build --release --locked --offline: passed; target/release/game-super-runtime.
- make install DESTDIR="$PWD/target/package-stage" PREFIX=/usr: passed; staged
  executable reports 0.2.0. New guide/schemas/examples included.
- bash tools/make-dist.sh: generated target/dist/game-super-runtime-0.2.0.tar.gz,
  SHA256SUMS, PKGBUILD and .SRCINFO. makepkg --verifysource: checksum passed.
- bash -n shell artifacts, both JSON schema syntax checks, git diff --check: passed.
- Removed owned /tmp/gsr-runtime-probe scratch copy after checking no process used
  it; no virtual-directory test mounts remain. Test/server processes completed.

Investigation details/limits:
- Host has bwrap 0.13.1 and usable FUSE; namespace probe passed without privilege.
- Installed fuse-overlayfs prints 'unknown argument ignored: auto_unmount' despite
  FUSE help mentioning it. Removed that option. Explicit owned-session unmount,
  service death signal and documented crash recovery are used instead.
- No actual WeMod/Windows app portability or modded commercial game tested; registry
  and services are not imported. Existing target directories required. No claim
  that mappings fix detached Wine launchers; no global wineserver killing.
- Ubuntu official docs confirm namespace restrictions separate from FUSE support;
  Ubuntu/Flatpak/Snap still need manual policy verification. Upstream links in guide.
- Existing detached-daemon/Windows lifecycle and UMU provisioning limitations remain
  explicit. Minimum Rust 1.85 and clean Arch chroot not retested on separate hosts.
