// SPDX-License-Identifier: Apache-2.0
/**
 * The plugin's PostToolUse hook (hooks/manifest-edit.mjs), exercised the way
 * Claude Code runs it: a separate Node process reading the edit on stdin.
 * It must speak only when a manifest edit changes a dependency version, and
 * must never fail or delay an edit it does not understand.
 */

import { describe, it, expect } from "vitest";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HOOK = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "hooks", "manifest-edit.mjs");

function run(payload: unknown): { status: number | null; stdout: string } {
  const res = spawnSync(process.execPath, [HOOK], {
    input: typeof payload === "string" ? payload : JSON.stringify(payload),
    encoding: "utf8",
    timeout: 10_000,
  });
  return { status: res.status, stdout: res.stdout };
}

function context(stdout: string): string {
  const parsed = JSON.parse(stdout) as { hookSpecificOutput: { hookEventName: string; additionalContext: string } };
  expect(parsed.hookSpecificOutput.hookEventName).toBe("PostToolUse");
  return parsed.hookSpecificOutput.additionalContext;
}

describe("manifest-edit hook", () => {
  it("names a crate upgrade and the exact upgrade_impact call", () => {
    const { status, stdout } = run({
      tool_name: "Edit",
      tool_input: { file_path: "src-tauri/Cargo.toml", old_string: 'fastembed = "5"\nserde = "1"', new_string: 'fastembed = "7.1.0"\nserde = "1"' },
    });
    expect(status).toBe(0);
    const text = context(stdout);
    expect(text).toContain("fastembed: 5 -> 7.1.0");
    expect(text).toContain('upgrade_impact {"package":"fastembed","to_version":"7.1.0","ecosystem":"crates.io"}');
    expect(text).not.toContain("serde");
  });

  it("reads package.json ranges and table-form Cargo versions, across MultiEdit edits", () => {
    const npm = run({
      tool_name: "MultiEdit",
      tool_input: {
        file_path: "package.json",
        edits: [
          { old_string: '"vite": "^6.0.0"', new_string: '"vite": "^7.0.0"' },
          { old_string: '"@tauri-apps/api": "2.11.1"', new_string: '"@tauri-apps/api": "2.12.0"' },
        ],
      },
    });
    const text = context(npm.stdout);
    expect(text).toContain("vite: ^6.0.0 -> ^7.0.0");
    expect(text).toContain('upgrade_impact {"package":"@tauri-apps/api","to_version":"2.12.0","ecosystem":"npm"}');
    // dependency_check gets the exact pin only; the vite range has no exact target.
    expect(text).toContain(
      'dependency_check {"items":[{"ecosystem":"npm","package":"@tauri-apps/api","to":"2.12.0","from":"2.11.1"}]}',
    );

    const cargo = run({
      tool_name: "Edit",
      tool_input: { file_path: "Cargo.toml", old_string: 'tokio = { version = "1.40", features = ["full"] }', new_string: 'tokio = { version = "1.47", features = ["full"] }' },
    });
    expect(context(cargo.stdout)).toContain("tokio: 1.40 -> 1.47");
    expect(context(cargo.stdout)).not.toContain("dependency_check");
  });

  it("points Python and Go edits at the briefing (upgrade_impact covers npm and crates.io)", () => {
    const { stdout } = run({
      tool_name: "Edit",
      tool_input: { file_path: "requirements.txt", old_string: "django==4.2.16", new_string: "django==5.1.1" },
    });
    const text = context(stdout);
    expect(text).toContain("django: ==4.2.16 -> ==5.1.1");
    expect(text).toContain("what_should_i_know");
  });

  it("stays silent for non-manifest files, unchanged versions, Write and bad input", () => {
    for (const payload of [
      { tool_name: "Edit", tool_input: { file_path: "src/main.rs", old_string: "a", new_string: "b" } },
      { tool_name: "Edit", tool_input: { file_path: "package.json", old_string: '"name": "x"', new_string: '"name": "y"' } },
      { tool_name: "Write", tool_input: { file_path: "package.json", content: '{"dependencies":{"vite":"^7.0.0"}}' } },
      "not json",
      {},
    ]) {
      const { status, stdout } = run(payload);
      expect(status).toBe(0);
      expect(stdout).toBe("");
    }
  });
});
