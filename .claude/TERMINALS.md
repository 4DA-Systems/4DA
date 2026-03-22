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
- **Commit Lock**: HELD | (omit if not held)
- **Files**: list of files being modified
-->

### T1 — Execute all phases: hygiene gates + screenshot moments + first-run excellence
- **Status**: working
- **Files**: scripts/check-file-sizes.cjs, src/store/feedback-slice.ts, src/components/Onboarding.tsx, src/components/FeedbackMilestone.tsx, src/components/WeeklyProgressCard.tsx, src/components/AccuracySparkline.tsx, src/components/DepHealthShield.tsx, src/components/FirstCveCard.tsx, src/components/GuidedHighlights.tsx

### T2 — Full application redesign (Waves 1A-3D)
- **Status**: done
