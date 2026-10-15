# Decisions

Record consequential choices when they occur. Keep routine implementation detail
in code or the work log. Give entries stable IDs so the checkpoint can link them.
When a choice changes, mark the old entry superseded and reference its replacement.

## Entry format

### D-001 — Short decision title

- Date: YYYY-MM-DD
- Status: Proposed / Accepted / Superseded by D-...
- Context: The problem and constraints that required a choice.
- Decision: What was chosen.
- Rationale: Evidence, alternatives considered, and relevant tradeoffs.
- Consequences: Follow-up work, limits, and conditions for revisiting the choice.

### D-001 — Rust host launcher and Bash in-container supervisor

- Date: 2026-10-15
- Status: Accepted
- Decision: Rust parses/validates/plans and supervises the host launch. For hooks
  and companions, generate a strictly quoted Bash supervisor placed after the
  detected Steam runtime boundary. All game-scope children share that invocation.
- Rationale: A host-built glibc-linked executable might not run in an older Steam
  container. Bash is already provided in Steam runtime environments. Plain launches
  pass the original argv straight through, with no shell expansion or env injection.
- Boundaries: Recognize known Steam wrappers only, never search arbitrary game args.
  Explicit runtime.command supports other installations. Proton companions use run;
  waitforexitandrun can deadlock while a companion keeps wineserver alive.

### D-002 — Local browser GUI with no additional system toolkit

- Date: 2026-10-15
- Status: Accepted
- Decision: Embed static HTML/JS and a small loopback HTTP server. Open using xdg-open
  or print URL. Common controls plus full YAML, validation, backup, conflict checks.
- Rationale: Keeps the game launcher free of GTK/Qt/WebKit package dependencies.
  A browser is only needed when the user chooses --edit-gui. Server never launches
  games or commands; URL token, custom header, Host and Origin checks protect writes.

### D-003 — Defaults are creation templates; precise argv and explicit expansion

- Date: 2026-10-15
- Status: Accepted
- Decision: New APP-ID.yaml copies defaults.yaml once. Existing files are standalone,
  never implicitly merged with later defaults. Commands are arrays; prepared Bash
  scripts are explicit and receive GSR_* target vars. YAML is executable user config.
- Rationale: Stable behavior and reviewable plans are necessary for complex setups.
  Relative configuration paths resolve under config directory; argv words are literal
  except explicit documented substitutions. Supplied Steam argv is never expanded.

### D-004 — Optional private mounts and persistent FUSE overlays

- Date: 2026-10-15
- Status: Accepted
- Decision: 0.2 adds opt-in bubblewrap directory binds and foreground fuse-overlayfs
  merges. Bubblewrap wraps the complete launch; original target trees are not
  modified. Require existing targets rather than creating placeholders through a
  writable root bind. Overlay writes persist in explicit upper/work storage.
- Evidence: Real bind/FUSE tests passed precedence, copy-up, deletion persistence,
  read-only/shared-writable semantics, signal cleanup, cross-app write locks,
  setup rollback and virtual-only executable/cwd. Actual SteamLinuxRuntime_sniper
  game/helper probes both saw the merged files; original content stayed unchanged.
- Contract: Optional external manifest v1 (YAML/JSON), literal relative-to-manifest
  paths, ordered enabled layers. Future adapters can emit it without a GSR update.
- Dependencies: No new Rust crates or unconditional tools. bwrap for binds;
  fuse-overlayfs/fusermount3/device access for overlays. No implicit root, global
  AppArmor weakening, registry migration, or modification of live user profiles.
- Limits: Existing external Wine services/detached processes may hold old mount
  views; Windows app portability and Ubuntu/Flatpak policy remain manual coverage.
  Overlay path delimiters/overlaps rejected. Crash recovery documented; no deletion
  of user write storage or recursive removal of mount trees.

### D-005 — Playtest follow-ups within existing lifecycle ownership

- Date: 2026-10-15
- Status: Accepted
- Decision: Add opt-in companion host_environment for Steam-to-desktop library
  restoration (explicit env still wins). TERM/grace/KILL remaining launch-group
  members even after the direct child exits. Preserve original exit status.
- Evidence: Playtest Kate loader failure and same-group gamescopereaper survival;
  new process tests reproduce and validate these improvements.
- Limits: Do not claim prefix-wide Windows cleanup or implement global wineserver
  killing. Independent companion lifetime/provisioning remain separate features.
