---
description: "SWANS analysis — three-perspective deep investigation of any topic"
argument-name: topic
---

# /swans — Sherlock, Wizard, Nerd Synthesis

Analyze the given topic through three distinct expert perspectives, then synthesize into actionable recommendations.

## Topic

$ARGUMENTS

## Ground Rules

**Cardinal rule:** Every finding must cite specific evidence. No vague assertions. If you can't point to concrete proof, don't state it.

**Anti-patterns — automatic failures:**
- "This could potentially be an issue" — either it IS or it ISN'T
- Restating the obvious as insight
- Recommendations without concrete next steps
- Filler paragraphs that say nothing

## Step 1: Anchor (3 sentences max)

State what you're analyzing and verify three critical assumptions before proceeding. Format:

> **Analyzing:** [one sentence — what and why it matters]
>
> **Assumption check:**
> 1. [assumption] — [verified/challenged, one line of evidence]
> 2. [assumption] — [verified/challenged, one line of evidence]
> 3. [assumption] — [verified/challenged, one line of evidence]

If any assumption is challenged, note how that changes the analysis direction. Then proceed.

## Step 2: Three Perspectives

Run each perspective fully before moving to the next.

---

### SHERLOCK — Investigation & Outsider Questioning

You are a detective who takes nothing at face value. Your job is to find what others miss by examining evidence AND questioning whether the entire framing is correct.

**Your toolkit:**
- Follow evidence chains — what does the data actually show vs. what people assume?
- Identify cargo cult patterns — are we doing this because it works, or because everyone does it?
- Find the gaps — what's conspicuously absent from the narrative?
- Challenge the frame — is this even the right question to ask?
- Zero-assumption mode — if an outsider with no context looked at this, what would confuse them?

**Output format:**

**Key findings** (3-5, each with evidence):
1. **[Finding]** — [specific evidence]. *So what:* [why this matters]

**Hidden assumptions exposed** (1-3):
- [Thing everyone takes for granted] — [why it deserves scrutiny]

---

### WIZARD — Innovation & Paradigm Breaks

You see possibilities invisible to conventional thinking. Your job is to find the non-obvious moves — the ones that change the game rather than play it better.

**Your toolkit:**
- What would a 10x approach look like here?
- What adjacent domain has already solved this?
- What constraint is everyone accepting that could be eliminated?
- What becomes possible if we invert the problem?

**Output format:**

**Opportunities** (2-4, ranked by impact):
1. **[Opportunity]** — [how it works, why it's non-obvious]. *Unlock:* [what this enables]

**Paradigm challenge** (1 — the single biggest "what if"):
- What if [contrarian reframe]? [Brief case for why this deserves serious consideration]

---

### NERD — Technical Depth & Edge Cases

You live in the details. Your job is to find the technical truth — the precise mechanics, the edge cases, the failure modes that surface only under real conditions.

**Your toolkit:**
- What are the actual numbers/specs/constraints?
- What breaks at scale, at the edges, under pressure?
- What's the second-order technical consequence no one's modeling?
- Where is the complexity hiding?

**Output format:**

**Technical reality** (3-5 findings, each precise):
1. **[Finding]** — [specific technical detail]. *Risk:* [what goes wrong if ignored]

**Edge cases** (2-3):
- [Scenario] — [what happens, how severe]

---

## Step 3: Synthesis

Combine all three perspectives into a unified view. This is the deliverable — everything above builds to this.

**Convergence:** Where do 2+ perspectives agree? (These are highest-confidence findings.)

**Tensions:** Where do perspectives disagree? (These reveal genuine trade-offs — name the trade-off explicitly.)

**Action plan** (ordered by priority):
1. **[Action]** — [what to do, why now, expected outcome, blast radius if wrong]. Owner: [who/what]
2. **[Action]** — ...
3. **[Action]** — ...

**One-line prognosis:** [Single sentence — what happens if we act on this vs. ignore it]

## Success Criteria

- Every finding has evidence
- Every recommendation is actionable (who does what by when)
- Innovation is present throughout, not bolted on at the end
- Total output is dense and scannable — no padding, no throat-clearing
