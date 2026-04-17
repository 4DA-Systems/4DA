# Document Hygiene

**Non-negotiable rules for where planning/strategy/audit documents live in the 4DA repo.** Enforced by `scripts/check-doc-location.cjs` in pre-commit. Violations block the commit.

This rule exists because internal planning docs leaking into the repo root is a recurring pattern (25+ such files accumulated before this framework was put in place, 2026-04-18). The cause is straightforward: Claude's default instinct when doing planning work is to persist the plan as a markdown file at the nearest convenient location — which is repo root. Without guardrails, those files get swept into subsequent commits (`git add -A`, agent-led commits, etc.) and end up in the public git history.

---

## The six rules

### 1. Repo root is for public-facing docs only

The only `*.md` files allowed at repo root:

```
README.md  CHANGELOG.md  LICENSE.md  CONTRIBUTING.md  CODE_OF_CONDUCT.md
SECURITY.md  CLAUDE.md  AGENTS.md  CONVENTIONS.md  TRADEMARKS.md
CLA.md  LINUX.md  NETWORK.md
```

Anything else at repo root that Claude creates is a bug, not a feature.

### 2. Three canonical homes for internal docs

| Home | Purpose | Tracked? |
|---|---|---|
| `.claude/plans/` | Session-local working plans, scratchpads, per-session artefacts | **Gitignored** (never committed) |
| `docs/strategy/` | Curated, user-approved strategy & architecture docs | Tracked (after explicit user approval per-file) |
| `docs/private/` | Confidential: launch timing, pricing, legal, competitive | **Gitignored** (never committed) |

### 3. Planning-doc protocol for Claude

Before writing any `*.md` at repo root whose filename contains any of:

> `PLAN`, `STRATEGY`, `AUDIT`, `CHECKLIST`, `ROADMAP`, `TRAJECTORY`, `VIRAL`, `LAUNCH`, `PRE-LAUNCH`, `FIRST-CONTACT`, `FORTIFICATION`, `EXECUTION`, `ASCENT`, `BATTLE`, `MASTER`, `BARRIER`, `IMPROVEMENTS`, `CRITICAL`, `DEVELOPER-OS`, `NOTIFICATION-INTELLIGENCE`, `INTELLIGENCE-CONSOLE`, `whats-next`, `NEXT-STEPS`, `MISSION_`, `SHIP_`

Claude **must** follow this order of preference:

1. **First choice**: use TodoWrite + in-conversation reasoning. No file gets written.
2. **Second choice**: if a persistent doc is genuinely needed, write to `.claude/plans/` (gitignored, never leaks).
3. **Third choice**: if the doc is curated enough that it belongs in the tracked strategy corpus, ask the user first, then write to `docs/strategy/`.
4. **Never**: write such a doc directly to repo root. The pre-commit gate will reject it.

### 4. The public-ok escape hatch

If a doc genuinely must live at repo root and its name matches a block pattern, add this marker to the first 10 lines:

```html
<!-- public-ok: <one-line reason> -->
```

The reason must be specific. *"Approved by user"* is not enough — name the reason: *"Public launch checklist linked from README"* or similar.

### 5. Gitignore patterns back up the gate

The `.gitignore` has a pattern block matching all block-listed filenames at repo root. This is belt-and-braces: the gate blocks the commit, and even if the gate is bypassed (`--no-verify`), the files don't get staged in the first place.

### 6. No `git commit --no-verify` to bypass this gate

`--no-verify` is reserved for genuine infrastructure failures where the hook itself is broken. Using it to get a disallowed doc into a commit is a violation of this rule. If the gate is wrong, fix the gate — don't route around it.

---

## Enforcement

- **Layer 1 — Location doctrine**: this file. Read by Claude; governs behaviour.
- **Layer 2 — Gitignore patterns**: `.gitignore` lines under "Internal planning docs". Prevents staging.
- **Layer 3 — Pre-commit gate**: `scripts/check-doc-location.cjs`, wired into `.husky/pre-commit`. Rejects commits.
- **Layer 4 — CLAUDE.md cross-reference**: the main CLAUDE.md points to this rule in the "Document Hygiene" section.
- **Layer 5 — Retrospective move**: the 30 legacy plan docs were moved to `.claude/plans/archive-2026-04-18/` on the day this framework shipped, and untracked with `git rm --cached`.
- **Layer 6 — Audit**: `git ls-files '*.md' | grep -vE '^(README|CHANGELOG|LICENSE|CONTRIBUTING|CODE_OF_CONDUCT|SECURITY|CLAUDE|AGENTS|CONVENTIONS|TRADEMARKS|CLA|LINUX|NETWORK)\.md$' | grep -v '/'` — should return empty for tracked root `.md`. If it ever returns a filename not on the public allowlist, the framework leaked.

---

## If a rule is wrong

As with `intelligence-doctrine.md`: these rules were written to prevent the specific failure mode of planning docs leaking into the repo root (observed 2026-04-18). If future learning invalidates a rule:

1. Write an ADR explaining which specific failure mode the rule no longer needs to prevent.
2. Cite what replaced it.
3. Only then update this document and the gate.

Rules do not quietly erode. They are retired deliberately or they stand.
