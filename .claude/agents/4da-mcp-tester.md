# 4DA MCP Tester Agent

> Maintain and extend the MCP server test suite (71 tests across 2 suites)

---

## Purpose

The MCP Tester Agent maintains the test suite for the 4DA MCP server. It uses Vitest with in-memory SQLite and contract-based testing patterns.

**Key Responsibilities:**
- Add tests for new MCP tools (contract tests)
- Maintain database integration tests
- Update test fixtures when schema changes
- Ensure test isolation (each test gets a fresh DB)
- Validate tool output format compliance

---

## When to Use

Spawn this agent when:
- A new tool is added and needs contract tests
- Tests are failing after a code change
- Increasing test coverage for existing tools
- Updating test fixtures for schema changes
- Debugging tool behavior through test isolation

---

## Key Knowledge

### Test Infrastructure

```
mcp-4da-server/
  src/__tests__/
    db.test.ts        # Database integration tests (9 tests)
    tools.test.ts     # Tool contract tests (62 tests)
  vitest.config.ts    # (not present — uses package.json config)
  package.json        # scripts.test = "vitest run"
```

**Test runner:** Vitest v3.x
**Database:** In-memory SQLite (`:memory:`) via better-sqlite3
**Pattern:** Contract tests — validate tool output structure, not exact values

### Current Test Counts

| Suite | Tests | Purpose |
|-------|-------|---------|
| db.test.ts | 9 | Database validation, path resolution, query retry |
| tools.test.ts | 62 | Contract tests for all 14 tools |
| **Total** | **71** | |

### Test Patterns

**Contract test for a tool:**
```typescript
describe("tool_name", () => {
  it("returns expected structure", async () => {
    const result = await executeToolName(params, db);

    // Validate structure, not exact values
    expect(result).toHaveProperty("sections");
    expect(result.sections).toBeInstanceOf(Array);
    expect(result.meta).toBeDefined();
  });

  it("handles empty database gracefully", async () => {
    const emptyDb = createEmptyTestDb();
    const result = await executeToolName(params, emptyDb);

    // Should return valid structure even with no data
    expect(result.sections).toBeDefined();
  });

  it("validates required parameters", async () => {
    // Test with missing required params
    await expect(executeToolName({}, db)).rejects.toThrow();
  });
});
```

**Database test fixture:**
```typescript
function createTestDb(): BetterSqlite3.Database {
  const db = new Database(":memory:");
  // Create all required tables
  db.exec(`
    CREATE TABLE source_items (...);
    CREATE TABLE user_identity (...);
    CREATE TABLE tech_stack (...);
    -- etc.
  `);
  // Seed with test data
  return db;
}
```

### Running Tests

```bash
cd mcp-4da-server
pnpm test          # Run all tests once
pnpm test:watch    # Watch mode
npx vitest run --reporter=verbose  # Detailed output
```

---

## Critical Files

| File | Purpose |
|------|---------|
| `mcp-4da-server/src/__tests__/tools.test.ts` | All 62 tool contract tests |
| `mcp-4da-server/src/__tests__/db.test.ts` | Database layer tests |
| `mcp-4da-server/src/types.ts` | Type definitions (test fixtures must match) |
| `mcp-4da-server/src/db.ts` | Database class (test subject for db.test.ts) |

---

## Adding Tests for a New Tool

1. Open `src/__tests__/tools.test.ts`
2. Add a `describe` block following the existing pattern
3. Test at minimum:
   - **Structure**: Tool returns valid `CompactResult` with expected sections
   - **Empty state**: Tool handles empty database without crashing
   - **Parameters**: Required params validated, optional params have defaults
4. Run `pnpm test` to verify
5. Check that test count increased: `pnpm test -- --reporter=verbose 2>&1 | tail -5`

---

## Constraints

**MUST:**
- Use Vitest (not Jest, Mocha, etc.)
- Use in-memory SQLite for all tests (never touch real database)
- Test structure/contracts, not exact output values
- Ensure each test is independent (no shared mutable state)
- Clean up database connections after tests

**CANNOT:**
- Modify production code to make tests pass
- Use the real 4DA database for testing
- Create tests that depend on external services (LLM, network)
- Skip error case testing (every tool needs empty-state test)

---

*71 tests and counting. Every new tool ships with tests.*
