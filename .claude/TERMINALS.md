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


### T1 — LLM Services Modernization
- **Status**: working
- **Files**:
  - src/components/settings/AIProviderSection.tsx
  - src/components/onboarding/setup-ai-provider.tsx
  - src/components/onboarding/use-quick-setup.ts
  - src/store/settings-slice.ts
  - src/locales/en/ui.json
  - src-tauri/src/llm.rs
  - src-tauri/src/llm_judge.rs
  - src-tauri/src/settings/mod.rs
  - src-tauri/src/settings/validation.rs
  - src-tauri/src/settings/env_detection.rs
  - src-tauri/src/settings_commands_llm.rs
  - mcp-4da-server/src/llm.ts
  - src/components/ScoreAutopsy.test.tsx

