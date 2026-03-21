# Authority Stack

**Hierarchical governance for software projects — built for teams that ship with AI.**

The Authority Stack is a lightweight framework of four Markdown files that give your project institutional memory, prevent re-litigation of settled decisions, and constrain AI coding assistants to operate within your project's rules.

Originally developed as the governance layer of [4DA](https://4da.ai), it works for any software project — open source or commercial, solo or team, with or without AI assistants.

---

## What Is the Authority Stack?

The Authority Stack is a strict hierarchy of four documents that govern how your project operates. Each layer has a specific purpose and a clear relationship to the others:

```
INVARIANTS.md          ← Non-negotiable constraints (highest authority)
  WISDOM.md            ← Principles, gates, and ways of working
    DECISIONS.md       ← Architectural decision log
      FAILURE_MODES.md ← Known fragile areas and antibodies (lowest authority)
```

**Higher layers override lower layers.** An invariant cannot be violated by a decision. A principle cannot be overridden by a failure mode mitigation. This hierarchy eliminates ambiguity.

## Why It Works

### 1. Prevents Re-litigation
Once a decision is recorded in `DECISIONS.md` with status ACCEPTED, it is settled. Anyone (human or AI) proposing to revisit it must first read the original context, alternatives considered, and consequences accepted. Most re-litigation dies here — the answer already exists.

### 2. Builds Institutional Memory
Every bug fix becomes an antibody in `FAILURE_MODES.md`. Every hard-won lesson becomes a principle in `WISDOM.md`. Every architectural choice is preserved in `DECISIONS.md`. Knowledge compounds instead of evaporating between sprints.

### 3. Constrains AI Assistants
When you point an AI coding assistant (Claude, Copilot, Cursor, etc.) at your `.ai/` directory, it operates within your project's rules instead of generic best practices. Your invariants become its invariants. Your decisions become its precedents. This is the difference between an AI that helps and an AI that undermines your architecture.

### 4. Scales Without Meetings
New team members read the Authority Stack and understand not just *what* the project does, but *why* it works the way it does and *what must never change*. Onboarding becomes self-service.

## Quick Start

**Time to set up: 10 minutes.**

1. **Copy the four template files** into your project:
   ```bash
   mkdir -p .ai
   cp INVARIANTS.md WISDOM.md DECISIONS.md FAILURE_MODES.md .ai/
   ```

2. **Fill in your invariants** — start with 3-5 things that must never break. Performance budgets, security requirements, data integrity rules. These are constraints, not goals.

3. **Record your first decision** — AD-001 is already written for you (adopting the Authority Stack). Add 2-3 more decisions that your team has already made but never documented.

4. **Add known failure modes** — think about the last 3 bugs that cost you real time. Write them up as antibodies so they never recur.

5. **Point your AI assistant** at the `.ai/` directory. Most AI coding tools support project-level instructions — reference these files there.

6. **Commit and iterate** — the Authority Stack is a living system. Update it as your project evolves.

### Recommended Directory Structure

```
your-project/
  .ai/
    INVARIANTS.md
    WISDOM.md
    DECISIONS.md
    FAILURE_MODES.md
  src/
  ...
```

The `.ai/` directory convention keeps governance files discoverable without cluttering the project root.

## How Each File Works

| File | Purpose | Update Frequency |
|------|---------|-----------------|
| `INVARIANTS.md` | Things that must never break | Rarely (quarterly review) |
| `WISDOM.md` | How the team works and decides | Monthly (as principles emerge) |
| `DECISIONS.md` | Why things are the way they are | Weekly (as decisions are made) |
| `FAILURE_MODES.md` | Known fragile areas and mitigations | After every significant bug |

## For AI-Assisted Development

If you use AI coding assistants, add this to your project's AI instructions (e.g., `CLAUDE.md`, `.cursorrules`, `.github/copilot-instructions.md`):

```markdown
## Authority Stack

Before modifying architecture or invariants, read the relevant `.ai/` file:
- `INVARIANTS.md` — non-negotiable system constraints
- `WISDOM.md` — principles and decision gates
- `DECISIONS.md` — architectural decisions log (prevents re-litigation)
- `FAILURE_MODES.md` — known fragile areas and previous regressions

Authority hierarchy: INVARIANTS > WISDOM > DECISIONS > FAILURE_MODES
```

This single paragraph transforms an AI assistant from a context-free code generator into a project-aware collaborator.

## Credits

The Authority Stack was created as the governance layer of [4DA](https://4da.ai) (4 Dimensional Autonomy), a privacy-first developer intelligence platform.

Learn more about the full 4DA Framework at [4da.ai/framework](https://4da.ai/framework).

## License

This template is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/) (Creative Commons Attribution 4.0 International).

You are free to use, modify, and distribute this template for any purpose — commercial or non-commercial — as long as you provide attribution:

```
Authority Stack template by 4DA (https://4da.ai)
```

The content you write in your own Authority Stack files is yours. This license applies only to the template structure and explanatory text.
