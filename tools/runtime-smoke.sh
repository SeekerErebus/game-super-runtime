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
      if [[ ${GSR_TEST_VIRTUAL:-0} == 1 ]]; then
        [[ $(cat "$GSR_CONFIG_DIR/../view/value") == overlay ]] || exit 1
      fi
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
if [[ ${GSR_TEST_VIRTUAL:-0} == 1 ]]; then
    mkdir -p "$test_dir/view" "$test_dir/mod"
    printf base > "$test_dir/view/value"
    printf overlay > "$test_dir/mod/value"
    cat >> "$test_dir/config/probe.yaml" <<YAML
virtual_directories:
  - type: overlay
    target: '$test_dir/view'
    write_dir: '$test_dir/writes'
    layers:
      - path: '$test_dir/mod'
YAML
fi
# Keep the runtime copy/state separate from the user's normal runtime state.
# A cross-filesystem location may copy instead of hard-linking the runtime.
export PRESSURE_VESSEL_VARIABLE_DIR=${GSR_RUNTIME_TEST_STATE:-/tmp/gsr-runtime-probe}
export PRESSURE_VESSEL_FILESYSTEMS_RW="$test_dir"
export PRESSURE_VESSEL_BATCH=1
export GSR_CONFIG_DIR="$test_dir/config"
export XDG_STATE_HOME="$test_dir/state"
"$binary" probe "$runtime" -- /bin/bash -c 'readlink /proc/self/ns/mnt > "$GSR_CONFIG_DIR/game.ns"; if [[ ${GSR_TEST_VIRTUAL:-0} == 1 ]]; then [[ $(cat "$GSR_CONFIG_DIR/../view/value") == overlay ]]; fi' > "$test_dir/stdout" 2> "$test_dir/stderr" || {
    tail -n 40 "$test_dir/stderr" >&2
    exit 1
}
if [[ ${GSR_TEST_VIRTUAL:-0} == 1 ]]; then
    [[ $(cat "$test_dir/view/value") == base ]]
    [[ -z $(ls -A "$test_dir/state/game-super-runtime/virtual-sessions") ]]
fi
cmp "$test_dir/config/helper.ns" "$test_dir/config/game.ns"
if [[ $(readlink /proc/self/ns/mnt) == "$(cat "$test_dir/config/game.ns")" ]]; then
    echo 'The command did not enter a different mount namespace' >&2
    exit 1
fi
printf 'Real Steam runtime smoke passed: game and companion share %s\n' "$(cat "$test_dir/config/game.ns")"
