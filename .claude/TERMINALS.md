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

### T1 — Morning brief 7-layer fix (score persistence + language firewall)
- **Status**: working
- **Since**: 2026-04-05T15:15:00Z
- **Files**:
  - src-tauri/src/types.rs
  - src-tauri/src/scoring/pipeline.rs
  - src-tauri/src/scoring/pipeline_tests.rs
  - src-tauri/src/scoring/analyzer.rs
  - src-tauri/src/scoring/benchmark.rs
  - src-tauri/src/scoring/validation/runner.rs
  - src-tauri/src/scoring/simulation/mod.rs
  - src-tauri/src/scoring/simulation/differential.rs
  - src-tauri/src/analysis_status.rs
  - src-tauri/src/analysis_deep_scan.rs
  - src-tauri/src/probes_engine.rs
  - src-tauri/src/toolkit_intelligence.rs
  - src-tauri/src/monitoring_briefing.rs
  - src-tauri/src/monitoring_jobs.rs
  - src-tauri/src/free_briefing.rs
  - src-tauri/src/digest_commands.rs
  - src-tauri/src/signal_terminal.rs
  - src-tauri/src/db/cache.rs
  - src-tauri/src/db/migrations.rs
  - src-tauri/src/language_detect.rs
  - src-tauri/src/source_fetching/processor.rs
  - src-tauri/src/content_translation_commands.rs
  - public/briefing.js
  - public/briefing.html
  - src/locales/en/ui.json
  - src/locales/es/ui.json

