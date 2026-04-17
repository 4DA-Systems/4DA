# Document Hygiene

**Non-negotiable rules for what documents may be tracked in the 4DA repo, and where they may live.** Enforced by `scripts/check-doc-location.cjs` in pre-commit and by `scripts/public-readiness-audit.cjs` on demand. Violations block the commit or fail the audit.

This rule exists because internal planning docs leaking into a *public* source repo is a recurring failure pattern. The v1 of this framework (2026-04-18) stopped the bleeding at the repo root; v2 (same day) closed the subdirectory escape routes (`.ai/`, `docs/strategy/`, `merch-print-ready/`, `docs/*ops*.md`) and added PII scanning.

---

## The seven rules

### 1. Root `.md` files match the public allowlist exactly

Only these `*.md` / top-level docs are allowed at repo root:

```
README.md  CHANGELOG.md  LICENSE  LICENSE.md  NOTICE
CONTRIBUTING.md  CODE_OF_CONDUCT.md  SECURITY.md
CLAUDE.md  AGENTS.md  CONVENTIONS.md
TRADEMARKS.md  CLA.md  LINUX.md  NETWORK.md
```

The canonical list lives in `scripts/doc-allowlist.json` under `root.files`. Any new root doc requires an explicit entry there, reviewed as part of the PR.

### 2. Mixed directories use a per-directory allowlist

Some directories legitimately contain a mix of public and private docs. For those, the allowlist is **positive** (everything not listed is blocked), not negative. Current mixed dirs:

| Directory | Purpose | Allowlist key in `doc-allowlist.json` |
|---|---|---|
| `.ai/` | Architecture / invariants / wisdom references | `.ai/` |
| `docs/strategy/` | Public architecture protocols and canonical schemas | `docs/strategy/` |

Adding a file to one of these dirs means adding it to the allowlist. No exceptions — the gate rejects unlisted files.

### 3. Three canonical homes for internal planning docs

| Home | Purpose | Tracked? |
|---|---|---|
| `.claude/plans/` | Session-local working plans, scratchpads | **Gitignored** |
| `docs/strategy/<ALLOWLISTED>` | Curated, architecture-only protocols | Tracked (per allowlist) |
| `docs/private/` | Confidential: pricing, launch timing, legal | **Gitignored** |

### 4. Planning-doc protocol for Claude

Before writing any `*.md` whose filename contains any of the block patterns (see the regex list in `scripts/check-doc-location.cjs`: `PLAN`, `STRATEGY`, `AUDIT`, `CHECKLIST`, `ROADMAP`, `TRAJECTORY`, `VIRAL`, `LAUNCH`, `PRE-LAUNCH`, `FIRST-CONTACT`, `FORTIFICATION`, `EXECUTION`, `ASCENT`, `BATTLE`, `MASTER`, `BARRIER`, `IMPROVEMENTS`, `CRITICAL`, `DEVELOPER-OS`, `NOTIFICATION-INTELLIGENCE`, `INTELLIGENCE-CONSOLE`, `whats-next`, `NEXT-STEPS`, `MISSION_`, `SHIP_`) Claude must follow this order of preference:

1. **First**: use TodoWrite + in-conversation reasoning. No file gets written.
2. **Second**: if a persistent doc is genuinely needed, write to `.claude/plans/` (gitignored).
3. **Third**: if it belongs in the tracked strategy corpus, ask the user first, then write to `docs/strategy/` AND add to `scripts/doc-allowlist.json`.
4. **Never**: write such a doc directly to repo root, to `.ai/`, or to a mixed dir without updating the allowlist.

### 5. PII never enters tracked content

Personal email addresses, home addresses, phone numbers that belong to the operator (not the business) must not appear in any tracked file. The gate enforces this for known personal identifiers:

- `runyourempirehq@gmail.com`
- `4dasystems@gmail.com`

Replace with role-based aliases (`hello@4da.ai`, `legal@4da.ai`, etc.) or move the file to a gitignored location. Business info that is **statutorily public** (ABN, ACN, trademark serial numbers) is fine on legal pages (terms, privacy, contact) but should not appear in ops/internal docs.

Extend the PII list in `scripts/check-doc-location.cjs` when new categories are identified. The audit script picks up the same list.

### 6. Escape hatches must be explicit and justified

Two escape hatches exist:

- **Public-OK marker**: add `<!-- public-ok: <one-line reason> -->` to the first 10 lines. Allowed for root + mixed-dir checks. **Never** allowed for PII — PII must be removed, not marked OK.
- **Allowlist entry**: add the filename to the appropriate block in `scripts/doc-allowlist.json`.

Both require a specific reason. *"Approved by user"* is not enough.

### 7. No `git commit --no-verify` to bypass this gate

`--no-verify` is reserved for genuine infrastructure failures. Routing around the gate is a policy violation. If the gate is wrong, fix the gate.

---

## Enforcement layers

| Layer | Artefact | Runs when |
|---|---|---|
| 1 Location doctrine | this file | Claude reads before writing docs |
| 2 Gitignore patterns | `.gitignore` (root + `.ai/` + `docs/strategy/` + `docs/` ops + `merch-print-ready/`) | Every stage attempt |
| 3 Pre-commit gate | `scripts/check-doc-location.cjs` via `.husky/pre-commit` | Every commit |
| 4 CLAUDE.md cross-ref | "Document Hygiene" section in CLAUDE.md | Every session start |
| 5 Allowlist source of truth | `scripts/doc-allowlist.json` | Consumed by layers 3 + 6 |
| 6 On-demand audit | `scripts/public-readiness-audit.cjs` via `pnpm run audit:public-ready` | Before publishing, periodic review |
| 7 Retrospective archive | `.claude/plans/archive-2026-04-18/` | Historical snapshot |

---

## How to run the audit

```bash
pnpm run audit:public-ready
```

The audit scans ALL tracked files (not just staged) and checks:

1. Root-level docs match the public allowlist
2. Mixed-directory allowlists (`.ai/`, `docs/strategy/`) are respected
3. No PII patterns anywhere
4. No suspicious filenames (secret, credential, `.env`, `.pem`, etc.)
5. No API key / JWT / private-key block patterns in content
6. Aggressive-phrasing spot-check (warn only)
7. `README.md` and `LICENSE` both present

Non-zero exit if any blocking finding is present. Required to pass before the repo is flipped to public.

---

## If a rule is wrong

These rules were written to prevent two specific failure modes observed in sequence on 2026-04-18:

- **v1 incident**: 30 planning docs (PLAN-*, VIRAL-STRATEGY, TRAJECTORY-PLAN, MASTER-PLAN, etc.) accumulated at repo root and entered git history.
- **v2 incident**: same pattern replicated under `.ai/` (22 audit/plan docs), `docs/strategy/` (12 strategic/tier-plan docs), `docs/` ops files (8), and `merch-print-ready/` (56 commerce prep files). Personal Gmail addresses also leaked into 4 tracked docs.

If future learning invalidates a rule:

1. Write an ADR explaining which specific failure mode the rule no longer needs to prevent.
2. Cite what replaced it.
3. Only then update this document, the gate, and the audit.

Rules do not quietly erode. They are retired deliberately or they stand.
