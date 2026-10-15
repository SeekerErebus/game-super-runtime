# Project brief

Status: Implemented and validated as version 0.1.0. Manual game/GPU coverage and publication remain release activities.

## Original assignment

The complete assignment is preserved in [ORIGINAL_BRIEF.md](ORIGINAL_BRIEF.md).

## Objective and scope

Build a dependable Linux game launcher, `game-super-runtime APP-ID [Steam command...]`,
configured by per-app YAML. Include CLI, friendly GUI editing, validation, process
lifecycle, extensible launch composition, documentation, and publication packaging.

## Requirements and constraints

- Rust preferred; minimal installed runtime dependencies suitable for Steam systems.
- Native and Steam launches; create missing YAML from configurable user defaults.
- Environment, Proton version/options/prefix, wrappers/overlays, prepared scripts,
  and simultaneous helper applications, including inside one pressure vessel.
- `APP-ID --edit` and `APP-ID --edit-gui` create missing configs and edit them.
- Basic actions should need minimal technical knowledge. Docs prioritize AI agents.
- Strict YAML validation, actionable errors/logging, signal handling and cleanup.
- Ready for GitHub and potential AUR packaging. No commit/push/publication requested.

## Acceptance criteria

1. Empty defaults preserve supplied argv, environment, output, and exit status.
2. First launch/edit creates one validated APP-ID.yaml; existing files are preserved.
3. Deterministic plans support native/Proton launches, arbitrary wrapper nesting,
   explicit env changes, hooks, companions, and shared-container boundaries.
4. Tests cover quoting, invalid input, custom defaults, failure propagation,
   process cleanup, signals, and actual simultaneous fake game/helper processes.
5. Text and graphical editing work; GUI exposes common controls and full YAML,
   validates before atomic save, protects concurrent edits, and stays local.
6. A newcomer can build, install, launch, edit, inspect a plan, and find logs.
   Include schema/reference/examples and reproducible package/CI instructions.
7. Run formatting, lint, tests, release build, and packaging smoke checks.
   Distinguish simulated Steam integration from real game verification.

## Milestones

1. Complete: runtime investigation, schema and launch plan, safe defaults/storage.
2. Complete: executor, scripts, shared runtime, companions and lifecycle tests.
3. Complete: editor/GUI, reference/examples, packaging and acceptance review.

## Assumptions and open questions

- GUI will use an embedded loopback web editor opened in the system browser,
  avoiding a mandatory GUI toolkit dependency. Launching games needs no browser.
- Installed overlays/Proton/runtimes remain user selected optional integrations.
- Cannot promise every third-party launcher layout; recognize supported boundaries,
  fail clearly on unsafe ambiguity, and provide explicit composition escape hatches.
- No sub-agents authorized by the user or workspace instructions.

## Requirement changes

None yet. Record later changes with a date and the requirement superseded.
