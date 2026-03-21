# Invariants

> **Authority Level: HIGHEST** — Nothing in WISDOM.md, DECISIONS.md, or FAILURE_MODES.md may contradict an invariant. Invariants are non-negotiable constraints, not aspirational goals. If something is negotiable, it belongs in WISDOM.md as a principle.

## What Is an Invariant?

An invariant is a property of your system that must **always** be true, under all conditions, with zero exceptions. Invariants are not best practices or guidelines — they are structural constraints that, if violated, mean the system is broken.

Good invariants are:
- **Testable** — you can write an automated check for them
- **Binary** — they are either satisfied or violated, no gray area
- **Non-negotiable** — violating one is a stop-ship bug, always

Bad invariants are:
- Goals ("Code should be clean")
- Preferences ("We prefer functional style")
- Aspirations ("Response times should be fast")

### How to Write an Invariant

State the constraint, not the goal. Include the verification method.

```
INV-XXX: [Short name]
Constraint: [What must always be true]
Verification: [How to check — automated test, CI check, manual audit]
Rationale: [Why this is non-negotiable — what breaks if violated]
```

---

## Performance Invariants

<!-- Example (uncomment and customize):

INV-001: Page Load Budget
Constraint: First contentful paint must complete within 2 seconds on a 3G connection.
Verification: Lighthouse CI check in pull request pipeline.
Rationale: Users abandon pages that take longer than 3 seconds. The 2-second budget
provides margin for real-world network variance.

-->

_Add your performance invariants here._

## Security Invariants

<!-- Example (uncomment and customize):

INV-010: No Secrets in Source
Constraint: No API keys, passwords, tokens, or credentials may exist in version-controlled files.
Verification: Pre-commit hook runs secret scanner (gitleaks/trufflehog). CI pipeline
runs the same scanner on every push.
Rationale: A single committed secret can compromise production infrastructure.
Secrets must be injected via environment variables or a secrets manager.

-->

_Add your security invariants here._

## Data Integrity Invariants

<!-- Example (uncomment and customize):

INV-020: Migrations Are Forward-Only
Constraint: Database migrations must never be modified after they have been applied to
any environment (including local dev). Corrections require a new migration.
Verification: CI compares migration file checksums against a locked manifest.
Rationale: Modified migrations create schema drift between environments, causing
data corruption that is extremely difficult to diagnose and repair.

-->

_Add your data integrity invariants here._

## UX Invariants

_Add your user experience invariants here. Examples: accessibility requirements, minimum contrast ratios, maximum interaction latency._

---

## Violation Protocol

When an invariant is violated:

1. **Stop** — do not ship, merge, or deploy until resolved
2. **Document** — record the violation in `FAILURE_MODES.md` with root cause
3. **Fix** — restore the invariant
4. **Harden** — add or improve the automated verification to prevent recurrence
5. **Review** — if the invariant itself was wrong, follow the amendment process below

## Amending Invariants

Invariants can be changed, but the bar is high:

1. Open a discussion (issue, RFC, or team sync) — not a drive-by edit
2. Document the rationale for change in `DECISIONS.md` as a formal decision
3. Update the invariant with a changelog note at the bottom of this file
4. Update all automated verifications to match the new constraint

---

_Authority Stack template by [4DA](https://4da.ai) — licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)_
