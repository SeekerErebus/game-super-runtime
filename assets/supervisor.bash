# Embedded by the Rust planner. All configured words are separately quoted.
# Bash job control gives every managed task its own process group without setsid.
set -u
set -o pipefail
set +m
gsr_groups=()
gsr_companions=()
gsr_required=()
gsr_names=()
gsr_stopping=0
gsr_main=0

gsr_log() {
    printf 'game-super-runtime: %s\n' "$*" >&2
    if [[ -n $gsr_log_file ]]; then
        { printf '%s\n' "$*" >> "$gsr_log_file"; } 2>/dev/null || :
    fi
}

gsr_start() {
    set -m
    ( trap - INT TERM HUP; "$@" ) &
    gsr_pid=$!
    set +m
    gsr_groups+=("$gsr_pid")
}

gsr_alive() { kill -0 "$1" 2>/dev/null; }

gsr_prune_groups() {
    local pid keep=()
    for pid in "${gsr_groups[@]}"; do
        if kill -0 -- "-$pid" 2>/dev/null; then keep+=("$pid"); fi
    done
    gsr_groups=("${keep[@]}")
}

gsr_check_main() {
    if [[ $gsr_main != 0 ]] && ! gsr_alive "$gsr_main"; then
        wait "$gsr_main"
        exit $?
    fi
}

gsr_stop_group() {
    kill -TERM -- "-$1" 2>/dev/null || :
}

gsr_cleanup() {
    local pid tick alive limit max=$gsr_shutdown_ticks
    (( gsr_main_grace_ticks > max )) && max=$gsr_main_grace_ticks
    gsr_prune_groups
    for pid in "${gsr_groups[@]}"; do gsr_stop_group "$pid"; done
    for ((tick=0; tick<max; tick++)); do
        alive=0
        for pid in "${gsr_groups[@]}"; do
            limit=$gsr_shutdown_ticks
            [[ $pid == "$gsr_main" ]] && limit=$gsr_main_grace_ticks
            if (( tick >= limit )); then kill -KILL -- "-$pid" 2>/dev/null || :; fi
            if kill -0 -- "-$pid" 2>/dev/null; then alive=1; fi
        done
        [[ $alive == 0 ]] && break
        sleep 0.05
    done
    for pid in "${gsr_groups[@]}"; do
        kill -KILL -- "-$pid" 2>/dev/null || :
        wait "$pid" 2>/dev/null || :
    done
    gsr_groups=()
}

gsr_check_companions() {
    local i pid rc
    for i in "${!gsr_companions[@]}"; do
        pid=${gsr_companions[$i]}
        [[ $pid == 0 ]] && continue
        if ! gsr_alive "$pid"; then
            wait "$pid"; rc=$?
            gsr_companions[$i]=0
            gsr_log "Companion '${gsr_names[$i]}' exited ($rc)."
            if [[ ${gsr_required[$i]} == 1 ]]; then
                gsr_log "Required companion stopped; shutting down the game."
                return 125
            fi
        fi
    done
    return 0
}

gsr_hook() {
    local label=$1 timeout=$2 allow=$3 fn=$4 pid rc=0 ticks=0 limit
    gsr_log "Starting hook: $label"
    gsr_start "$fn"
    pid=$gsr_pid
    limit=$(( (timeout + 49) / 50 ))
    while gsr_alive "$pid"; do
        if [[ $timeout != 0 && $ticks -ge $limit ]]; then
            gsr_log "Hook timed out: $label"
            gsr_stop_group "$pid"
            sleep 0.05
            kill -KILL -- "-$pid" 2>/dev/null || :
            wait "$pid" 2>/dev/null || :
            rc=124
            break
        fi
        sleep 0.05
        ((ticks+=1))
    done
    if [[ $rc == 0 ]]; then wait "$pid"; rc=$?; fi
    gsr_prune_groups
    if [[ $rc != 0 ]]; then
        gsr_log "Hook '$label' failed ($rc)."
        [[ $allow == 1 ]] && return 0
    fi
    return "$rc"
}

gsr_companion() {
    local label=$1 required=$2 delay=$3 ready=$4 timeout=$5 fn=$6
    local tick pid limit rc
    # Delay before this companion starts; order in YAML defines startup order.
    limit=$(( (delay + 49) / 50 ))
    for ((tick=0; tick<limit; tick++)); do
        gsr_check_main
        gsr_check_companions || return $?
        sleep 0.05
    done
    gsr_log "Starting companion: $label"
    gsr_start "$fn"
    pid=$gsr_pid
    gsr_companions+=("$pid")
    gsr_required+=("$required")
    gsr_names+=("$label")
    if [[ -n $ready ]]; then
        limit=$(( (timeout + 49) / 50 ))
        for ((tick=0; tick<limit; tick++)); do
            gsr_check_main
            gsr_check_companions || return $?
            [[ -e $ready ]] && return 0
            if ! gsr_alive "$pid"; then break; fi
            sleep 0.05
        done
        gsr_log "Companion '$label' did not create its ready_file within ${timeout}ms."
        gsr_stop_group "$pid"
        [[ $required == 1 ]] && return 125
    fi
    return 0
}

gsr_finish() {
    local rc=$1 after_rc=0
    [[ $gsr_stopping == 1 ]] && return
    gsr_stopping=1
    trap '' INT TERM HUP
    trap - EXIT
    gsr_cleanup
    export GSR_EXIT_CODE=$rc
    gsr_after || after_rc=$?
    gsr_cleanup
    if [[ $rc == 0 && $after_rc != 0 ]]; then rc=$after_rc; fi
    exit "$rc"
}

trap 'gsr_finish 130' INT
trap 'gsr_finish 143' TERM
trap 'gsr_finish 129' HUP
trap 'gsr_finish $?' EXIT
