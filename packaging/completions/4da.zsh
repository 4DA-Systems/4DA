#compdef 4da

# Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
# Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.
#
# Zsh completion for 4DA CLI
# Install: copy to a directory in your $fpath (e.g., /usr/share/zsh/site-functions/_4da)
#   or add to fpath in .zshrc: fpath=(/path/to/completions $fpath)

_4da() {
    local -a commands
    local -a global_opts

    global_opts=(
        '--help[Show help information]'
        '-h[Show help information]'
        '--version[Show version]'
    )

    commands=(
        'briefing:Show latest AI briefing'
        'brief:Show latest AI briefing (alias)'
        'b:Show latest AI briefing (alias)'
        'signals:Show items with signal classifications'
        'signal:Show items with signal classifications (alias)'
        's:Show items with signal classifications (alias)'
        'gaps:Show knowledge gaps in your dependencies'
        'gap:Show knowledge gaps (alias)'
        'g:Show knowledge gaps (alias)'
        'health:Show project dependency health'
        'h:Show project dependency health (alias)'
        'status:Show database stats'
        'st:Show database stats (alias)'
        'help:Show help information'
    )

    _arguments -C \
        $global_opts \
        '1:command:->command' \
        '*::arg:->args'

    case "$state" in
        command)
            _describe -t commands 'command' commands
            ;;
        args)
            case "${words[1]}" in
                signals|signal|s)
                    _arguments \
                        '--critical[Only show critical/high priority signals]' \
                        '-c[Only show critical/high priority signals]' \
                        '--help[Show help]' \
                        '-h[Show help]'
                    ;;
                briefing|brief|b|gaps|gap|g|health|h|status|st)
                    _arguments \
                        '--help[Show help]' \
                        '-h[Show help]'
                    ;;
            esac
            ;;
    esac
}

_4da "$@"
