# Architectural Decisions

> **Authority Level: STANDARD** — Decisions are subordinate to both INVARIANTS.md and WISDOM.md. A decision cannot override an invariant or contradict a principle. Decisions override mitigations in FAILURE_MODES.md.

## What Is an Architectural Decision?

An architectural decision is any choice that:
- Is difficult or expensive to reverse later
- Affects multiple parts of the system
- Has trade-offs that future team members will wonder about

Recording decisions prevents **re-litigation** — the wasteful pattern where teams repeatedly debate settled questions because no one wrote down why the original choice was made.

## Decision Statuses

| Status | Meaning |
|--------|---------|
| **ACCEPTED** | Active and in effect. This is how the project works. |
| **SUPERSEDED** | Replaced by a newer decision. The superseding decision ID is noted. |
| **DEPRECATED** | No longer applicable. The system has changed enough that this decision is irrelevant. |

## Decision Format

```
## AD-XXX: [Decision Title]

**Date:** YYYY-MM-DD
**Status:** ACCEPTED | SUPERSEDED by AD-YYY | DEPRECATED

### Context

[What situation prompted this decision? What problem were you solving?
Include enough detail that someone unfamiliar with the project can
understand the trade-offs.]

### Decision

[What did you decide? State it clearly and concisely.]

### Alternatives Considered

1. **[Alternative A]** — [Why it was rejected]
2. **[Alternative B]** — [Why it was rejected]

### Consequences

- [Positive consequence]
- [Negative consequence or accepted trade-off]
- [What this enables or prevents going forward]
```

---

## Decisions

### AD-001: Adopt the Authority Stack

**Date:** <!-- Fill in today's date -->
**Status:** ACCEPTED

#### Context

As the project grows, architectural knowledge is scattered across pull request descriptions, Slack threads, meeting notes, and individual memory. New team members cannot understand *why* the system works the way it does. The same debates recur every few months because previous conclusions were never recorded. AI coding assistants operate without awareness of project-specific constraints, sometimes generating code that violates unwritten rules.

#### Decision

Adopt the Authority Stack — a four-file governance framework placed in the `.ai/` directory:
- `INVARIANTS.md` for non-negotiable constraints
- `WISDOM.md` for principles and decision gates
- `DECISIONS.md` (this file) for architectural decisions
- `FAILURE_MODES.md` for known fragile areas and mitigations

All team members (and AI assistants) are expected to consult these files before making changes that affect architecture, invariants, or known failure modes.

#### Alternatives Considered

1. **ADR tools (adr-tools, MADR)** — Focused only on decisions, missing the invariants, principles, and failure modes layers that prevent the most costly mistakes. The Authority Stack is a superset.
2. **Wiki/Notion pages** — Disconnected from the codebase. Documentation that lives outside the repo drifts out of sync within weeks. Co-locating governance with code keeps it honest.
3. **No formal documentation** — The status quo. Relies on tribal knowledge and individual memory. Breaks down with team changes, extended breaks, and AI-assisted development.

#### Consequences

- Team has a single, version-controlled source of truth for project governance
- New contributors can onboard by reading four files instead of months of chat history
- AI assistants operate within project-specific constraints when pointed at `.ai/`
- Requires discipline to update — decisions must be recorded as they are made, not retroactively in bulk
- Small overhead per decision (~5 minutes to write up) that prevents hours of re-litigation

---

<!-- Add your decisions below. Use sequential IDs: AD-002, AD-003, etc. -->

---

_Authority Stack template by [4DA](https://4da.ai) — licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)_
