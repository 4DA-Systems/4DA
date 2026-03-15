# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **After committing**: Remove your entry.
4. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

<!-- Add entries below. Format:
### T[N] — [short description]
- **Status**: working | committing | done
- **Files**: list of files being modified
-->

### T1 — P1 Linux improvements
- **Status**: done
- **Files**: src-tauri/src/suns/hardware_monitor.rs, src-tauri/src/bin/cli.rs, LINUX.md

### T2 — P0 Linux fixes (NVIDIA, single-instance, desktop, CI)
- **Status**: done
- **Files**: src-tauri/src/lib.rs, src-tauri/Cargo.toml, src-tauri/tauri.conf.json, src-tauri/desktop-template.desktop, .github/workflows/release.yml
