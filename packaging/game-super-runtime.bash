_game_super_runtime() {
    local cur=${COMP_WORDS[COMP_CWORD]} prev=${COMP_WORDS[COMP_CWORD-1]}
    if [[ $prev == --config-dir ]]; then
        mapfile -t COMPREPLY < <(compgen -d -- "$cur")
    elif [[ $cur == -* ]]; then
        mapfile -t COMPREPLY < <(compgen -W '--help --version --config-dir --list --edit-defaults --edit-defaults-gui --edit --edit-gui --no-open --validate --dry-run --doctor --config-path --logs' -- "$cur")
    elif (( COMP_CWORD == 1 )); then
        local dir=${GSR_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/game-super-runtime} file id
        COMPREPLY=()
        for file in "$dir"/*.yaml; do
            [[ -f $file ]] || continue
            id=${file##*/}; id=${id%.yaml}
            [[ $id != defaults && $id == "$cur"* ]] && COMPREPLY+=("$id")
        done
    fi
}
complete -F _game_super_runtime game-super-runtime
