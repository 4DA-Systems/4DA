# Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
# Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.
#
# Bash completion for 4DA CLI
# Install: copy to /usr/share/bash-completion/completions/4da
#   or source from ~/.bashrc: source /path/to/4da.bash

_4da() {
    local cur prev commands
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Top-level commands (full names and aliases)
    commands="briefing brief b signals signal s gaps gap g health h status st help"

    # Global flags
    local global_flags="--help -h --version"

    case "${prev}" in
        4da)
            # Complete commands and global flags
            COMPREPLY=( $(compgen -W "${commands} ${global_flags}" -- "${cur}") )
            return 0
            ;;
        signals|signal|s)
            # Signals subcommand flags
            COMPREPLY=( $(compgen -W "--critical -c --help -h" -- "${cur}") )
            return 0
            ;;
        briefing|brief|b|gaps|gap|g|health|h|status|st)
            # These commands have no subcommand-specific flags beyond --help
            COMPREPLY=( $(compgen -W "--help -h" -- "${cur}") )
            return 0
            ;;
    esac

    # If current word starts with a dash, offer flags
    if [[ "${cur}" == -* ]]; then
        COMPREPLY=( $(compgen -W "${global_flags}" -- "${cur}") )
        return 0
    fi

    # Default: offer commands
    COMPREPLY=( $(compgen -W "${commands}" -- "${cur}") )
    return 0
}

complete -F _4da 4da
