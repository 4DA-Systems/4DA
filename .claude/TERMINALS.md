# Terminal Coordination

## Protocol
1. **Before editing**: Read this file. If your files are claimed by another terminal, STOP.
2. **Claim files**: Add your entry below with the files you'll touch.
3. **Commit Lock**: Set `**Commit Lock**: HELD` before committing. Only one terminal commits at a time.
4. **After committing**: Remove your entry and release the lock.
5. **Conflicts**: If two terminals touch the same file, the one that committed first wins. The other must rebase.

## Active Terminals

<!-- Add entries below. Format:
### T[N] — [short description]
- **Status**: working | committing | done
- **Since**: ISO timestamp (e.g., 2026-03-22T10:00:00Z)
- **Commit Lock**: HELD | (omit if not held)
- **Files**: list of files being modified
-->

### T1 — Security Hardening (Fortress Wave)
- **Status**: working
- **Since**: 2026-04-07T12:00:00Z
- **Files**:
  - Wave 1 (Rust security): src-tauri/src/ipc_guard.rs, src-tauri/src/utils/url.rs, src-tauri/src/embeddings.rs, src-tauri/src/context_commands.rs
  - Wave 2 (Supply chain): src-tauri/deny.toml, src-tauri/rust-toolchain.toml, .github/workflows/validate.yml
  - Wave 3 (CI hardening): .github/workflows/release.yml, SECURITY.md

