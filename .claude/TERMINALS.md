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
- **Status**: working
- **Files**: Wave 1A: deleting DigestView, CommunityInsights, WisdomPulse, AppHeader, BriefingMetrics, BriefingTopPicks, SignalActionCard, BriefingSupplementary, BriefingCard, BriefingHelpers cleanup. Wave 1C: ViewRouter.tsx, SovereignDeveloperProfile.tsx. Wave 1B: App.tsx, ActionBar.tsx, NaturalLanguageSearch.tsx, ViewTabBar.tsx, EmbeddingStatusIndicator.tsx, new UnifiedAppBar.tsx. Wave 2-3: AttentionCards.tsx, MilestoneOverlay.tsx, BriefingWarmupState.tsx, BriefingEmptyStates.tsx, IntelligenceFeed.tsx, use-keyboard-shortcuts.ts, PulseSummary.tsx, intelligence_history.rs
