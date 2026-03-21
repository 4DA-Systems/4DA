# Wisdom

> **Authority Level: HIGH** — Principles in this file govern how the team works and makes decisions. They are subordinate to INVARIANTS.md but override individual decisions in DECISIONS.md and mitigations in FAILURE_MODES.md.

## What Is Project Wisdom?

Wisdom is the accumulated knowledge of *how* your team works best. Unlike invariants (which define what must never break), principles define how you approach problems, make trade-offs, and avoid known pitfalls.

Principles are:
- **Earned** — they come from experience, not theory
- **Opinionated** — they take a stance on how to work
- **Evolving** — they can be refined as the team learns

### How to Write a Principle

State the principle, then explain why it exists and what it prevents.

```
P-XXX: [Principle name]
Statement: [The principle in one sentence]
Context: [Why this principle exists — what pain it prevents]
```

---

## Principles

<!-- Example principles (uncomment and customize):

P-001: Explicit Over Clever
Statement: Choose the explicit, readable approach over the clever, compact one.
Context: Clever code impresses during code review but costs 10x during debugging.
Every abstraction must justify its cognitive overhead. When in doubt, be boring.

P-002: Own Your Dependencies
Statement: Understand every dependency you add. If you cannot explain what it does
and why you need it, do not add it.
Context: Transitive dependencies are the leading source of supply chain vulnerabilities
and unexpected breaking changes. A 2KB utility function you write is safer than a
popular package you do not understand.

P-003: Failures Are Data
Statement: Every production failure must produce a FAILURE_MODES.md entry within 48 hours.
Context: The only thing worse than a bug is a bug that recurs because no one documented
the first occurrence. Failure modes are the project's immune system — they only work
if you feed them.

-->

_Add your principles here._

---

## Zero Zones

Zero Zones are **structural impossibilities** — things the system is designed to make impossible, not merely unlikely. They are stronger than "don't do this" guidelines because the architecture itself prevents them.

### What Makes a Good Zero Zone?

A Zero Zone is not a rule that people must follow. It is a constraint enforced by code, infrastructure, or process so that the prohibited outcome *cannot happen* even if someone tries.

```
ZZ-XXX: [Zero Zone name]
Impossible outcome: [What cannot happen]
Enforcement: [How the architecture prevents it — code, infra, or process]
```

<!-- Example:

ZZ-001: No Unencrypted Secrets at Rest
Impossible outcome: Plaintext secrets stored in the database or filesystem.
Enforcement: The settings module encrypts all secret-type values before writing.
The database schema has no plaintext secret columns. The pre-commit hook blocks
any file matching known secret patterns.

-->

_Add your zero zones here._

---

## Wisdom Gates

Wisdom Gates are **decision checkpoints** — moments where the team must pause and verify before proceeding. They prevent costly mistakes by inserting a mandatory review step at high-risk transitions.

### When to Define a Gate

Add a gate when:
- The action is irreversible or expensive to reverse
- The action affects users in production
- Past experience shows this is where mistakes happen

```
Gate N: [Gate name]
Trigger: [When this gate activates]
Required checks: [What must be verified before proceeding]
```

<!-- Example:

Gate 1: Pre-Merge
Trigger: Before merging any pull request to main.
Required checks:
  - All CI checks pass (tests, linting, type checking)
  - At least one approval from a team member who did not write the code
  - No unresolved comments marked as "blocking"
  - DECISIONS.md updated if the PR changes architecture

Gate 2: Pre-Deploy
Trigger: Before deploying to production.
Required checks:
  - Staging environment tested with production-like data
  - Rollback plan documented (how to revert if something breaks)
  - On-call engineer identified and available

-->

_Add your wisdom gates here._

---

## Anti-Paralysis Checklist

Analysis paralysis kills projects. When the team is stuck debating a decision, use this checklist to break the deadlock:

- [ ] **Is this reversible?** If yes, pick the simpler option, ship it, and learn. Reversible decisions do not deserve lengthy debate.
- [ ] **Is there a clear winner on maintenance cost?** The option that is easier to maintain over 2 years usually wins, even if it is harder to build initially.
- [ ] **Are we debating preferences or constraints?** Preferences are negotiable. Constraints are in INVARIANTS.md. If the debate is about preferences, timebox it to 15 minutes and decide.
- [ ] **Do we have data?** If not, can we get data in less time than we have spent debating? Run the experiment instead of arguing.
- [ ] **What is the cost of delay?** Sometimes shipping an imperfect solution now is better than shipping a perfect solution next month. But sometimes it is not. Be honest about which case you are in.
- [ ] **Record and move on.** Whatever you decide, write it in DECISIONS.md with the alternatives you rejected and why. This prevents the same debate from recurring.

---

_Authority Stack template by [4DA](https://4da.ai) — licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)_
