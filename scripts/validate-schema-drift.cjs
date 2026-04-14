/**
 * validate-schema-drift.cjs
 *
 * Detects schema drift between test-setup CREATE TABLE strings and the
 * production migration chain. The `is_direct` column bug this week was
 * exactly this: migration 53 added a column to project_dependencies, but
 * 5+ test files had CREATE TABLE strings without it. Tests "passed"
 * against incomplete schemas while production code used columns the tests
 * didn't know about.
 *
 * How it works:
 *   1. Parse every CREATE TABLE from src-tauri/src/db/migrations.rs
 *      (the canonical source of truth for production schema)
 *   2. Parse every CREATE TABLE from #[cfg(test)] blocks in all .rs files
 *   3. For each table that appears in both: compare column sets
 *   4. Flag any test schema that's MISSING a column present in production
 *
 * Usage:
 *   node scripts/validate-schema-drift.cjs           # informational
 *   node scripts/validate-schema-drift.cjs --strict   # exit 1 on violations
 *
 * Limitations:
 *   - Parses SQL as text, not via a real SQL parser. Works for the
 *     standard CREATE TABLE patterns 4DA uses; may miss exotic syntax.
 *   - Only detects MISSING columns (test has fewer than production).
 *     Does not detect EXTRA columns (test has more) — that's a different
 *     and less dangerous class of drift.
 *   - Does not handle ALTER TABLE — only CREATE TABLE definitions.
 */

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const STRICT = process.argv.includes("--strict");

// ---------------------------------------------------------------------------
// SQL parsing helpers
// ---------------------------------------------------------------------------

/**
 * Extract all CREATE TABLE statements from a string, returning a map of
 * table_name -> Set<column_name>.
 *
 * Handles multi-line statements. Only extracts column names, not types —
 * we're checking for MISSING columns, not type mismatches.
 */
function extractTableColumns(sql) {
  const tables = {};

  // Match CREATE TABLE [IF NOT EXISTS] <name> ( ... )
  // The regex is permissive about whitespace and case.
  const createTableRegex =
    /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(([\s\S]*?)\)/gi;

  let m;
  while ((m = createTableRegex.exec(sql)) !== null) {
    const tableName = m[1].toLowerCase();
    const body = m[2];

    // Extract column names: each column definition starts with a name
    // (word chars) followed by a type keyword or comma.
    // Skip constraints: PRIMARY KEY, UNIQUE, FOREIGN KEY, CHECK, etc.
    const columns = new Set();
    const lines = body.split(/,\s*/).map((l) => l.trim());
    for (const line of lines) {
      // Skip constraint lines (they start with keywords, not column names)
      if (
        /^\s*(PRIMARY\s+KEY|UNIQUE|FOREIGN\s+KEY|CHECK|CONSTRAINT)/i.test(line)
      ) {
        continue;
      }
      // Extract the first word as the column name
      const colMatch = line.match(/^(\w+)/);
      if (colMatch) {
        const col = colMatch[1].toLowerCase();
        // Reject pure numbers (column names cannot start with a digit in SQL).
        // These sneak in as fragments from DEFAULT 0, DEFAULT 1, etc. when a
        // column definition wraps or contains commas inside a default expr.
        if (/^\d+$/.test(col)) continue;
        // Skip SQL keywords that aren't column names. Includes wrap-line
        // tokens (DEFAULT, REFERENCES, AUTOINCREMENT, COLLATE, ON, CASCADE,
        // NO, ACTION, SET, NULL, NOT) that can appear as the first token of
        // a fragment when a column definition spans commas/lines.
        if (
          ![
            "create",
            "table",
            "if",
            "not",
            "exists",
            "insert",
            "into",
            "values",
            "select",
            "default",
            "references",
            "autoincrement",
            "collate",
            "on",
            "cascade",
            "no",
            "action",
            "set",
            "null",
          ].includes(col)
        ) {
          columns.add(col);
        }
      }
    }

    if (columns.size > 0) {
      // If we see the same table multiple times (e.g., in different
      // migration steps), merge columns (production schema accumulates).
      if (!tables[tableName]) {
        tables[tableName] = columns;
      } else {
        for (const col of columns) {
          tables[tableName].add(col);
        }
      }
    }
  }

  return tables;
}

// ---------------------------------------------------------------------------
// File scanning
// ---------------------------------------------------------------------------

function walkDir(dir, extensions) {
  const results = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (/node_modules|target[\\/]/.test(full)) continue;
    if (entry.isDirectory()) {
      results.push(...walkDir(full, extensions));
    } else if (entry.isFile()) {
      if (extensions.includes(path.extname(entry.name))) {
        results.push(full);
      }
    }
  }
  return results;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  console.log("=== 4DA Schema Drift Validator ===\n");

  // 1. Parse production schema from migrations.rs
  const migrationsPath = path.join(
    ROOT,
    "src-tauri",
    "src",
    "db",
    "migrations.rs"
  );
  let migrationsContent;
  try {
    migrationsContent = fs.readFileSync(migrationsPath, "utf-8");
  } catch (e) {
    console.error(`Cannot read migrations.rs: ${e.message}`);
    process.exit(1);
  }
  const prodTables = extractTableColumns(migrationsContent);
  const prodTableNames = Object.keys(prodTables);
  console.log(
    `Production tables (from migrations.rs): ${prodTableNames.length}`
  );

  // 2. Find all test CREATE TABLE definitions across all .rs files
  const rustFiles = walkDir(path.join(ROOT, "src-tauri", "src"), [".rs"]);
  const testSchemas = []; // { file, line, tableName, columns: Set }

  for (const file of rustFiles) {
    // Skip migrations.rs itself — that's the source of truth
    if (file.endsWith("migrations.rs")) continue;

    let content;
    try {
      content = fs.readFileSync(file, "utf-8");
    } catch {
      continue;
    }

    // Only look at #[cfg(test)] blocks and test schemas
    // We can't perfectly scope to test blocks, but checking for
    // "CREATE TABLE" in files that also contain "#[cfg(test)]" or
    // "const TEST_SCHEMA" is a good heuristic.
    if (
      !content.includes("CREATE TABLE") ||
      (!content.includes("#[cfg(test)]") && !content.includes("TEST_SCHEMA") && !content.includes("#[test]"))
    ) {
      continue;
    }

    const tables = extractTableColumns(content);
    for (const [tableName, columns] of Object.entries(tables)) {
      // Find the approximate line number for reporting
      const idx = content.indexOf(`CREATE TABLE`);
      const line =
        idx >= 0 ? content.substring(0, idx).split("\n").length : 0;
      testSchemas.push({
        file: path.relative(ROOT, file),
        line,
        tableName,
        columns,
      });
    }
  }

  console.log(
    `Test schemas found: ${testSchemas.length} table definitions across ${new Set(testSchemas.map((s) => s.file)).size} files\n`
  );

  // 3. Compare: for each test schema, check if it's missing columns
  //    from the production version of the same table.
  const violations = [];

  for (const ts of testSchemas) {
    const prodCols = prodTables[ts.tableName];
    if (!prodCols) continue; // Test table not in production — custom test fixture, fine.

    const missing = [];
    for (const col of prodCols) {
      if (!ts.columns.has(col)) {
        missing.push(col);
      }
    }

    if (missing.length > 0) {
      violations.push({
        file: ts.file,
        line: ts.line,
        table: ts.tableName,
        missing,
        testCols: [...ts.columns].sort(),
        prodCols: [...prodCols].sort(),
      });
    }
  }

  // 4. Report
  if (violations.length > 0) {
    console.log(`--- Schema drift detected (${violations.length}) ---\n`);
    for (const v of violations) {
      console.log(`  ${v.file}:${v.line}`);
      console.log(`    Table: ${v.table}`);
      console.log(`    Missing from test: ${v.missing.join(", ")}`);
      console.log(
        `    Test has:    [${v.testCols.join(", ")}]`
      );
      console.log(
        `    Prod has:    [${v.prodCols.join(", ")}]`
      );
      console.log();
    }

    if (STRICT) {
      console.log(
        `--strict: ${violations.length} schema drift violation(s) found, exiting 1`
      );
      process.exit(1);
    }
  } else {
    console.log(
      "No schema drift detected. All test schemas include every production column.\n"
    );
  }
}

main();
