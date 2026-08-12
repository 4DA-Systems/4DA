# Task Specification Template
## Use This Format When Defining Tasks

---

## Why Use This Template

Vague tasks produce vague results. This template ensures:
- Clear scope boundaries
- Identified constraints
- Known risks surfaced
- Success criteria defined

---

## Template

Copy this and fill in for each task:

```markdown
## Task: [Short descriptive name]

### Goal
[One sentence: What are we trying to achieve?]

### Success Criteria
- [ ] [Specific, measurable outcome 1]
- [ ] [Specific, measurable outcome 2]
- [ ] [How do we know this is done?]

### Files Allowed to Change
- `path/to/file1.rs` - [what change]
- `path/to/file2.ts` - [what change]

### Files Forbidden to Change
- `path/to/critical.rs` - [why protected]
- (anything not explicitly allowed)

### Relevant Invariants
- INV-XXX: [invariant name] - [how it applies]
- INV-YYY: [invariant name] - [how it applies]

### Known Risks
- FM-XXX: [failure mode] - [mitigation approach]

### Dependencies
- [What must be true before starting?]
- [What other systems/files are involved?]

### Out of Scope
- [What this task explicitly does NOT include]
- [Adjacent work that should be separate tasks]

### Notes
[Any additional context, constraints, or considerations]
```

---

## Example: Well-Specified Task

```markdown
## Task: Add Reddit Source Adapter

### Goal
Enable 4DA to fetch and process posts from specified subreddits.

### Success Criteria
- [ ] Reddit adapter fetches posts from user-configured subreddits
- [ ] Posts are deduplicated via content hash
- [ ] Rate limiting respects Reddit API guidelines (60 req/min)
- [ ] Adapter gracefully handles authentication failures
- [ ] Integration tests pass

### Files Allowed to Change
- `src-tauri/src/sources/mod.rs` - add reddit module export
- `src-tauri/src/sources/reddit.rs` - new file, implement adapter
- `src-tauri/src/settings/mod.rs` - add reddit configuration fields

### Files Forbidden to Change
- `src-tauri/src/db/` - schema changes require separate task
- `src-tauri/src/ace/*` - ACE logic unrelated to this task

### Relevant Invariants
- INV-003: ACE Never Fails Silently - all errors must be logged
- INV-004: ACE Respects Privacy - no user data sent to Reddit
- INV-031: BYOK Integrity - API credentials stored locally only

### Known Risks
- FM-MED-003: API Rate Limit - implement token bucket
- FM-MED-006: Content Extraction - handle various post formats

### Dependencies
- Reddit API credentials available via BYOK
- Source adapter trait defined in sources/mod.rs

### Out of Scope
- Reddit authentication UI (separate task)
- Comment threading (v2 feature)
- Subreddit discovery/recommendation

### Notes
- Start with read-only access (no posting)
- Consider NSFW filtering for content safety
```

---

## Example: Poorly-Specified Task (Don't Do This)

```markdown
## Task: Fix the Reddit thing

### Goal
Make Reddit work better.

### Files to Change
- Whatever needs changing

### Notes
- Should be pretty easy
```

**Why this is bad:**
- No success criteria
- No scope boundaries
- No invariants identified
- No risks acknowledged
- "Pretty easy" is never true

---

## When to Use This Template

**Use for:**
- Any task touching production code
- Features with multiple files
- Bug fixes with unclear scope
- Refactoring efforts

**Skip for:**
- Typo fixes (single character)
- Documentation-only changes
- Exploratory research

---

## Relationship to Two-Phase Protocol

This template IS Phase 1 output. Before executing:

1. **Phase 1 (Orientation):** Fill out this template
2. **Review:** Get approval on the specification
3. **Phase 2 (Execution):** Implement within spec boundaries
4. **Validation:** Verify against success criteria

---

*A task without a specification is a task without boundaries.*
