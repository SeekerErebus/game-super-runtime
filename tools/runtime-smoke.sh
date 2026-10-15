#!/usr/bin/env bash
# Explicit opt-in test. Supply an installed SteamLinuxRuntime/run executable.
set -euo pipefail
[[ $# == 1 && -x $1 ]] || { echo 'Usage: bash tools/runtime-smoke.sh /path/SteamLinuxRuntime_sniper/run' >&2; exit 2; }
runtime=$1
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
binary="$PWD/target/debug/game-super-runtime"
[[ -x $binary ]] || { echo 'Run cargo build first' >&2; exit 2; }
test_dir=$(mktemp -d)
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/config"
cat > "$test_dir/config/probe.yaml" <<'YAML'
version: 1
runtime:
  mode: required
scripts:
  helper:
    body: |
      readlink /proc/self/ns/mnt > "$GSR_CONFIG_DIR/helper.ns"
      touch "$GSR_CONFIG_DIR/helper.ready"
      trap 'exit 0' TERM
      while :; do sleep .1; done
companions:
  - name: Namespace probe
    script: helper
    ready_file: '${CONFIG_DIR}/helper.ready'
    required: true
shutdown_timeout_ms: 200
YAML
# Keep the runtime copy/state separate from the user's normal runtime state.
# A cross-filesystem location may copy instead of hard-linking the runtime.
export PRESSURE_VESSEL_VARIABLE_DIR=${GSR_RUNTIME_TEST_STATE:-/tmp/gsr-runtime-probe}
export PRESSURE_VESSEL_FILESYSTEMS_RW="$test_dir"
export PRESSURE_VESSEL_BATCH=1
export GSR_CONFIG_DIR="$test_dir/config"
export XDG_STATE_HOME="$test_dir/state"
"$binary" probe "$runtime" -- /bin/bash -c 'readlink /proc/self/ns/mnt > "$GSR_CONFIG_DIR/game.ns"' > "$test_dir/stdout" 2> "$test_dir/stderr" || {
    tail -n 40 "$test_dir/stderr" >&2
    exit 1
}
cmp "$test_dir/config/helper.ns" "$test_dir/config/game.ns"
if [[ $(readlink /proc/self/ns/mnt) == "$(cat "$test_dir/config/game.ns")" ]]; then
    echo 'The command did not enter a different mount namespace' >&2
    exit 1
fi
printf 'Real Steam runtime smoke passed: game and companion share %s\n' "$(cat "$test_dir/config/game.ns")"
