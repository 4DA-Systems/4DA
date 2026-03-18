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

### T1 — Privacy & Terms pages
- **Status**: done
- **Files**: site/src/privacy.njk, site/src/terms.njk, site/vercel.json, site/src/index.njk, site/src/_includes/streets-body.njk, site/src/merch.njk

### T2 — Developer OS Plan Implementation
- **Status**: working
- **Files**: DEVELOPER-OS-PLAN.md, src-tauri/src/dependencies.rs, src-tauri/src/dependency_health.rs, src-tauri/src/standing_queries.rs, src-tauri/src/temporal_graph.rs, src-tauri/src/accuracy.rs, src-tauri/src/ai_costs.rs, src-tauri/src/tech_convergence.rs, src-tauri/src/sources/cve.rs, editors/vscode/4da/, src/components/DependencyDashboard.tsx, src/components/SecurityDashboard.tsx, src/components/IntelligenceReport.tsx, src/components/StandingQueries.tsx
