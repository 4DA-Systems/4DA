// SPDX-License-Identifier: Apache-2.0
/**
 * Schema-vs-code contract: every parameter an executor reads is declared in
 * the schema hosts are shown, and every declared parameter is read.
 *
 * The drift this exists for (2026-10-02, verified live): agent_memory's
 * published schema said `topic` where the executor reads `query`, and left
 * out the `subject` that `store` requires — the tool could not be called
 * correctly by any client that followed its schema. decision_memory's
 * update/supersede parameters were missing from it too. Nothing compared the
 * two, so 425 green tests sat beside an unusable tool.
 *
 * Static by design: it reads each executor's source for `params.<name>` and
 * `const { a, b } = params`, which is how every executor here takes its input.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { TOOL_REGISTRY } from "../schema-registry.js";
import { validateToolArgs } from "../tool-args.js";

const toolsDir = join(dirname(fileURLToPath(import.meta.url)), "..", "tools");

/** The source file(s) whose code reads each tool's params. */
const SOURCES: Record<string, string[]> = {
  vulnerability_scan: ["vulnerability-scan.ts"],
  dependency_health: ["dependency-health.ts"],
  upgrade_planner: ["upgrade-planner.ts"],
  upgrade_impact: ["upgrade-impact.ts"],
  dependency_check: ["dependency-check.ts"],
  what_should_i_know: ["what-should-i-know.ts"],
  ecosystem_pulse: ["ecosystem-pulse.ts"],
  get_context: ["get-context.ts"],
  get_relevant_content: ["get-relevant-content.ts"],
  get_actionable_signals: ["get-actionable-signals.ts"],
  knowledge_gaps: ["knowledge-gaps.ts"],
  record_feedback: ["record-feedback.ts"],
  decision_memory: ["decision-memory.ts"],
  check_decision_alignment: ["decision-enforcement.ts"],
  agent_memory: ["agent-memory.ts"],
  developer_dna: ["developer-dna.ts"],
};

function paramsRead(files: string[]): Set<string> {
  const read = new Set<string>();
  for (const file of files) {
    const src = readFileSync(join(toolsDir, file), "utf8");
    for (const m of src.matchAll(/\bparams\??\.(\w+)/g)) read.add(m[1]);
    for (const m of src.matchAll(/const\s*\{([^}]+)\}\s*=\s*params\b/g)) {
      for (const part of m[1].split(",")) {
        const name = part.trim().split(/[:=\s]/)[0];
        if (name) read.add(name);
      }
    }
  }
  return read;
}

describe("schema-vs-code contract", () => {
  it("maps every registered tool to its executor source", () => {
    expect(Object.keys(SOURCES).sort()).toEqual(Object.keys(TOOL_REGISTRY).sort());
  });

  for (const [name, files] of Object.entries(SOURCES)) {
    it(`${name}: declared parameters == parameters the executor reads`, () => {
      const declared = new Set(Object.keys(TOOL_REGISTRY[name].definition.inputSchema.properties ?? {}));
      const read = paramsRead(files);
      const undeclared = [...read].filter((p) => !declared.has(p));
      const unread = [...declared].filter((p) => !read.has(p));
      expect(undeclared, `${name} reads parameters its schema does not declare`).toEqual([]);
      expect(unread, `${name} declares parameters its executor never reads`).toEqual([]);
      for (const required of TOOL_REGISTRY[name].definition.inputSchema.required ?? []) {
        expect(declared.has(required), `${name}: required "${required}" is not a property`).toBe(true);
      }
    });
  }

  it("agent_memory recall and store are callable exactly as the schema describes", () => {
    const schema = TOOL_REGISTRY.agent_memory.definition.inputSchema;
    expect(validateToolArgs("agent_memory", schema, { action: "recall", query: "fastembed" })).toBeNull();
    expect(validateToolArgs("agent_memory", schema, { action: "store", subject: "s", content: "c" })).toBeNull();
    expect(validateToolArgs("agent_memory", schema, { action: "recall", topic: "x" })).toMatch(/unknown parameter "topic"/);
  });
});

describe("argument validation", () => {
  const vuln = TOOL_REGISTRY.vulnerability_scan.definition.inputSchema;

  it("accepts a valid call and an empty one", () => {
    expect(validateToolArgs("vulnerability_scan", vuln, {})).toBeNull();
    expect(validateToolArgs("vulnerability_scan", vuln, { severity_filter: "high", include_dev: true })).toBeNull();
  });

  it("names the fix for a wrong type, a bad enum value and an unknown parameter", () => {
    expect(validateToolArgs("vulnerability_scan", vuln, { project_path: 42 })).toMatch(/"project_path" must be a string, got number 42/);
    expect(validateToolArgs("vulnerability_scan", vuln, { severity_filter: "severe" })).toMatch(/must be one of "critical", "high", "medium", "low"/);
    expect(validateToolArgs("vulnerability_scan", vuln, { severity: "high" })).toMatch(/unknown parameter "severity".*severity_filter/);
  });

  it("reports a missing required parameter", () => {
    const briefing = TOOL_REGISTRY.what_should_i_know.definition.inputSchema;
    expect(validateToolArgs("what_should_i_know", briefing, {})).toMatch(/missing required parameter "task"/);
  });
});
