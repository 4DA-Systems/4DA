// SPDX-License-Identifier: Apache-2.0
/**
 * Route and pattern syntax in string literals: the axum 0.8 path-parameter
 * change (`/:single` -> `/{single}`) touches `.route("/users/:id", ...)`, which
 * imports nothing that symbol matching could see.
 */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { flagOldSyntax, oldSyntaxShapes, stringLiterals } from "../tools/upgrade-impact-literals.js";
import type { ReportSection } from "../tools/upgrade-impact-report.js";

// Verbatim from axum 0.8.0's CHANGELOG.md (crate archive).
const AXUM_ENTRY =
  "**breaking:** Upgrade matchit to 0.8, changing the path parameter syntax from `/:single` and `/*many` to `/{single}` and `/{*many}`; the old syntax produces a panic to avoid silent change in behavior ([#2645])";

describe("oldSyntaxShapes", () => {
  it("takes the retired syntax from the 'from' side only", () => {
    const shapes = oldSyntaxShapes(AXUM_ENTRY);
    expect(shapes).toHaveLength(2);
    expect(shapes.some((s) => s.test("/users/:id"))).toBe(true);
    expect(shapes.some((s) => s.test("/files/*path"))).toBe(true);
    // The new syntax is what a migrated project already has; it is not flagged.
    expect(shapes.some((s) => s.test("/users/{id}"))).toBe(false);
  });

  it("leaves code paths, calls and plain names to symbol matching", () => {
    expect(oldSyntaxShapes("Removed `Router::route_service` and `res.json()` and `config`")).toEqual([]);
    expect(oldSyntaxShapes("The `*` wildcard must be named")).toEqual([]);
  });
});

describe("stringLiterals", () => {
  it("finds double, single and template literals with their lines", () => {
    const src = 'let a = 1;\nRouter::new().route("/users/:id", get(h));\nconst b = `/x/${y}`;\nconst c = \'/q\';';
    expect(stringLiterals(src)).toEqual([
      { text: "/users/:id", line: 2 },
      { text: "/x/${y}", line: 3 },
      { text: "/q", line: 4 },
    ]);
  });
});

describe("flagOldSyntax", () => {
  let root: string;
  beforeAll(() => {
    root = fs.mkdtempSync(path.join(os.tmpdir(), "upgrade-literals-"));
    fs.mkdirSync(path.join(root, "src"));
    fs.writeFileSync(path.join(root, "src", "main.rs"), 'use axum::Router;\nfn app() -> Router {\n    Router::new().route("/users/:id", get(user))\n}\n');
    fs.writeFileSync(
      path.join(root, "src", "migrated.rs"),
      'use axum::Router;\nfn app() -> Router {\n    Router::new().route("/users/{id}", get(user))\n}\n' +
        // Code inside a string (a test fixture) is not a route, even with `/*x` in it.
        'const FIXTURE: &str = "use anyhow::Result;\\n/*comment*/ let x = 1;";\n',
    );
  });
  afterAll(() => fs.rmSync(root, { recursive: true, force: true }));

  const sections = (): ReportSection[] => [
    {
      version: "0.8.0",
      date: null,
      entries: [
        { kind: "breaking", text: AXUM_ENTRY },
        { kind: "change", text: "Use `/:x` somewhere in an unrelated note" },
      ],
    },
  ];

  it("flags the breaking entry with the file and line of the old-syntax route", async () => {
    const s = sections();
    const result = await flagOldSyntax(s, [{ path: "src/main.rs" }, { path: "src/migrated.rs" }], root);
    expect(result.newlyTouching).toBe(1);
    expect(result.examples).toEqual(['"/users/:id"']);
    expect(s[0].entries[0].touches_your_code).toBe(true);
    expect(s[0].entries[0].matched_literals).toEqual([{ file: "src/main.rs", line: 3, literal: "/users/:id" }]);
    // Only breaking and deprecation entries are searched.
    expect(s[0].entries[1].matched_literals).toBeUndefined();
  });

  it("does nothing for a migrated project or without a project root", async () => {
    const migrated = sections();
    expect((await flagOldSyntax(migrated, [{ path: "src/migrated.rs" }], root)).newlyTouching).toBe(0);
    expect(migrated[0].entries[0].touches_your_code).toBeUndefined();
    expect((await flagOldSyntax(sections(), [{ path: "src/main.rs" }], null)).newlyTouching).toBe(0);
  });
});
