# Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
# Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.
#
# Fish completion for 4DA CLI
# Install: copy to ~/.config/fish/completions/4da.fish
#   or /usr/share/fish/vendor_completions.d/4da.fish

# Disable file completion by default
complete -c 4da -f

# Global flags
complete -c 4da -n "__fish_use_subcommand" -s h -l help -d "Show help information"
complete -c 4da -n "__fish_use_subcommand" -l version -d "Show version"

# Commands
complete -c 4da -n "__fish_use_subcommand" -a briefing -d "Show latest AI briefing"
complete -c 4da -n "__fish_use_subcommand" -a brief -d "Show latest AI briefing (alias)"
complete -c 4da -n "__fish_use_subcommand" -a b -d "Show latest AI briefing (alias)"

complete -c 4da -n "__fish_use_subcommand" -a signals -d "Show items with signal classifications"
complete -c 4da -n "__fish_use_subcommand" -a signal -d "Show items with signal classifications (alias)"
complete -c 4da -n "__fish_use_subcommand" -a s -d "Show items with signal classifications (alias)"

complete -c 4da -n "__fish_use_subcommand" -a gaps -d "Show knowledge gaps in your dependencies"
complete -c 4da -n "__fish_use_subcommand" -a gap -d "Show knowledge gaps (alias)"
complete -c 4da -n "__fish_use_subcommand" -a g -d "Show knowledge gaps (alias)"

complete -c 4da -n "__fish_use_subcommand" -a health -d "Show project dependency health"
complete -c 4da -n "__fish_use_subcommand" -a h -d "Show project dependency health (alias)"

complete -c 4da -n "__fish_use_subcommand" -a status -d "Show database stats"
complete -c 4da -n "__fish_use_subcommand" -a st -d "Show database stats (alias)"

complete -c 4da -n "__fish_use_subcommand" -a help -d "Show help information"

# Signals subcommand flags
complete -c 4da -n "__fish_seen_subcommand_from signals signal s" -l critical -d "Only critical/high priority signals"
complete -c 4da -n "__fish_seen_subcommand_from signals signal s" -s c -d "Only critical/high priority signals"
complete -c 4da -n "__fish_seen_subcommand_from signals signal s" -s h -l help -d "Show help"

# Other subcommand flags (help only)
complete -c 4da -n "__fish_seen_subcommand_from briefing brief b gaps gap g health h status st" -s h -l help -d "Show help"
