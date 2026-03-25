# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

### T1 — Data Health System
- **Status**: working
- **Since**: 2026-03-25T15:00:00Z
- **Files**:
  - src-tauri/src/db/history.rs
  - src-tauri/src/autophagy_commands.rs
  - src-tauri/src/lib.rs
  - src-tauri/src/monitoring.rs
  - src/components/settings/MonitoringSection.tsx
  - src/components/settings/DataHealthSection.tsx (new)
  - src/types/index.ts (or analysis.ts — data health types)
