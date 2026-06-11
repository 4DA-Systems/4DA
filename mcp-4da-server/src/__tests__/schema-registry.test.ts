// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Regression tests for the tool registry descriptions.
 *
 * Locks in the prescriptive "call-when" trigger guarantee: every tool the
 * server advertises must tell the calling model WHEN to use it, not just what
 * it does. Recent models reach for tools more conservatively and select them
 * more reliably when the description carries an explicit trigger condition.
 *
 * Without this test a future edit could silently strip the triggers (or let the
 * slim list bloat back past its token budget) while the contract tests stay
 * green. Guards both description surfaces: the slim ListTools summary and the
 * lazy-loaded full schema.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { TOOL_REGISTRY, getSlimToolList } from "../schema-registry.js";

// An explicit trigger: "Call [this] when/before/after/first/to ..." (case-insensitive).
// The optional "this" allows the natural "Call this before starting..." phrasing.
const TRIGGER = /\bCall (this )?(when|after|before|first|to)\b/i;
const schemaDir = join(dirname(fileURLToPath(import.meta.url)), "..", "schemas");

describe("tool registry descriptions", () => {
  it("registers exactly 14 tools (9 standalone, 5 full-mode)", () => {
    expect(Object.keys(TOOL_REGISTRY)).toHaveLength(14);
    expect(getSlimToolList().length).toBe(14);
    expect(getSlimToolList(true).length).toBe(9);
    expect(getSlimToolList(false).length).toBe(5);
  });

  it("every slim summary carries an explicit call-when trigger", () => {
    for (const [name, entry] of Object.entries(TOOL_REGISTRY)) {
      expect(entry.summary, `${name}: slim summary missing a 'Call ...' trigger`).toMatch(
        TRIGGER,
      );
    }
  });

  it("slim summaries stay within the token budget (guard against bloat)", () => {
    for (const [name, entry] of Object.entries(TOOL_REGISTRY)) {
      expect(
        entry.summary.length,
        `${name}: slim summary too long (${entry.summary.length} chars)`,
      ).toBeLessThanOrEqual(240);
    }
    // Combined slim-list budget. The pre-trigger baseline was ~2000 chars; with
    // triggers it sits near ~2450. Hard-cap well below the ~3200 over-budget
    // version so a future regression is caught.
    const total = Object.values(TOOL_REGISTRY).reduce((n, e) => n + e.summary.length, 0);
    expect(total, `combined slim list is ${total} chars`).toBeLessThanOrEqual(2800);
  });

  it("every full schema description also carries a trigger", () => {
    for (const [name, entry] of Object.entries(TOOL_REGISTRY)) {
      const raw = readFileSync(join(schemaDir, entry.schemaFile), "utf8");
      const desc = JSON.parse(raw).description as string;
      expect(desc, `${entry.schemaFile}: full description missing a trigger`).toMatch(TRIGGER);
    }
  });
});
