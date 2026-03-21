# Failure Modes

> **Authority Level: LOWEST** — Failure modes document known risks and mitigations. They are subordinate to all other Authority Stack files. A mitigation strategy must not violate an invariant, contradict a principle, or conflict with an accepted decision.

## What Is a Failure Mode?

A failure mode is a documented way the system can break, along with the knowledge needed to prevent or recover from it. Think of this file as your project's **immune system** — every significant bug you fix should leave an antibody here.

### The Immune System Concept

Biological immune systems work by remembering threats. After the body fights off an infection, it creates antibodies — specialized cells that recognize and neutralize that specific threat if it ever returns.

Your project works the same way:

1. **A bug occurs** — something breaks in production or during development
2. **You fix it** — the immediate problem is resolved
3. **You create an antibody** — you document the failure mode, its trigger, and how to prevent recurrence

Without step 3, the same class of bug will recur. Maybe in a different file, maybe triggered by a different input, but the same root cause. The failure mode entry is what prevents that.

### When to Add a Failure Mode

Add an entry when:
- A bug took more than 30 minutes to diagnose
- A bug affected users in production
- A bug was caused by a non-obvious interaction between components
- You find yourself saying "we need to remember this"

## Severity Levels

| Severity | Meaning | Response |
|----------|---------|----------|
| **CRITICAL** | Data loss, security breach, or complete system failure | Immediate fix. Stop all other work. Post-mortem required. |
| **HIGH** | Major feature broken, significant user impact | Fix within 24 hours. Update mitigation strategy. |
| **MEDIUM** | Feature degraded, workaround exists | Fix within current sprint. Document workaround. |
| **LOW** | Minor issue, cosmetic, or edge case | Fix when convenient. Document for awareness. |

## Failure Mode Format

```
## FM-XXX: [Short descriptive name]

**Severity:** CRITICAL | HIGH | MEDIUM | LOW
**First Observed:** YYYY-MM-DD
**Status:** ACTIVE | MITIGATED | RESOLVED

### Description

[What goes wrong? Describe the failure in concrete terms.]

### Trigger

[What causes this failure? Be specific — include the sequence of
events, conditions, or inputs that reproduce it.]

### Root Cause

[Why does this happen? What is the underlying technical reason?]

### Mitigation

[How to prevent this failure. Include both immediate fixes and
structural changes that make recurrence impossible.]

### Verification

[How to confirm the mitigation works. Ideally an automated test
or CI check.]
```

---

## Failure Modes

<!-- Example (uncomment and customize):

### FM-001: Race Condition in Cache Invalidation

**Severity:** HIGH
**First Observed:** 2025-01-15
**Status:** MITIGATED

#### Description

Users occasionally see stale data after updating their profile. The cached
profile object is not invalidated before the confirmation page renders.

#### Trigger

1. User submits profile update form
2. Backend writes to database (takes ~200ms)
3. Frontend immediately fetches profile for confirmation page
4. Cache returns stale data because invalidation has not propagated

Reproduces ~15% of the time under normal load, ~80% under high load.

#### Root Cause

The cache invalidation message is sent asynchronously after the database
write completes, but the frontend fetch can arrive at the cache before
the invalidation message does.

#### Mitigation

- Immediate: Added cache-busting query parameter to the confirmation page fetch
- Structural: Moved to read-after-write consistency by having the update
  endpoint return the updated object directly, bypassing the cache for
  the confirmation page

#### Verification

- Integration test `test_profile_update_consistency` verifies the confirmation
  page always shows updated data
- Load test scenario reproduces the original race condition to verify the fix
  holds under stress

-->

_Add your failure modes here._

---

## Periodic Review

Review this file quarterly:

- **Promote patterns** — if 3+ failure modes share a root cause, that root cause may deserve an invariant in INVARIANTS.md or a principle in WISDOM.md
- **Retire resolved entries** — mark long-resolved entries as RESOLVED to keep the active list focused
- **Check mitigations** — verify that automated checks referenced in mitigations still exist and pass

---

_Authority Stack template by [4DA](https://4da.ai) — licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)_
