#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0
/**
 * Claude Code plugin hook (PostToolUse on Edit / MultiEdit / Write): when an
 * edit changes a dependency's version in a manifest, tell the agent which
 * packages moved and to check upgrade_impact before it builds.
 *
 * Why a hook and not only a tool: an agent calls a tool when it thinks to.
 * Measured 2026-10-01, the agent asked to "bump fastembed 5 -> 7" got no
 * fastembed-specific help from the briefing it chose to call; a hook fires on
 * the edit itself. Developers asked for exactly this ("If this was a claude
 * plugin with a hook on my dep files, I'd be in", HN, 2026).
 *
 * Fast and quiet by design: no network, no dependencies, plain Node. Any
 * input it does not understand exits 0 with no output, so it can never block
 * or slow an edit. It reads only the edit payload Claude Code passes on stdin.
 */

import { basename } from "node:path";

const MANIFESTS = new Set(["package.json", "Cargo.toml", "pyproject.toml", "requirements.txt", "go.mod"]);

/** name -> version specifier pairs a manifest snippet declares. */
function declaredVersions(file, text) {
  const out = new Map();
  if (!text) return out;
  if (file === "package.json") {
    for (const m of text.matchAll(/"(@?[a-z0-9][\w.\-/]*)"\s*:\s*"([~^<>=]*\s*v?\d[^"]*)"/gi)) out.set(m[1], m[2]);
  } else if (file === "Cargo.toml") {
    for (const m of text.matchAll(/^\s*([A-Za-z_][\w-]*)\s*=\s*"([~^<>=]*\s*\d[^"]*)"/gm)) out.set(m[1], m[2]);
    for (const m of text.matchAll(/^\s*([A-Za-z_][\w-]*)\s*=\s*\{[^}]*\bversion\s*=\s*"([^"]+)"/gm)) out.set(m[1], m[2]);
  } else if (file === "pyproject.toml" || file === "requirements.txt") {
    for (const m of text.matchAll(/["']?([A-Za-z0-9][A-Za-z0-9._-]*)(?:\[[^\]]*\])?\s*(===?|~=|>=)\s*([0-9][^"',;\s]*)/g)) out.set(m[1].toLowerCase(), `${m[2]}${m[3]}`);
    for (const m of text.matchAll(/^\s*([A-Za-z0-9][A-Za-z0-9._-]*)\s*=\s*"([~^<>=]*\s*\d[^"]*)"/gm)) out.set(m[1].toLowerCase(), m[2]);
  } else if (file === "go.mod") {
    for (const m of text.matchAll(/^\s*(?:require\s+)?([a-z0-9.-]+\.[a-z]{2,}\/\S+)\s+(v\d\S*)/gm)) out.set(m[1], m[2]);
  }
  return out;
}

/** Changed or added dependency versions between two snippets of one manifest. */
function versionChanges(file, before, after) {
  const old = declaredVersions(file, before);
  const changes = [];
  for (const [name, spec] of declaredVersions(file, after)) {
    const prev = old.get(name);
    if (prev !== spec) changes.push({ name, from: prev ?? null, to: spec });
  }
  return changes;
}

const exact = (spec) => /^v?\d+\.\d+\.\d+([-+][\w.]+)?$/.test(spec.replace(/^[=\s]+/, "")) ? spec.replace(/^[=v\s]+/, "") : null;

function adviceFor(file, changes) {
  const ecosystem = file === "package.json" ? "npm" : file === "Cargo.toml" ? "crates.io" : null;
  const lines = changes.slice(0, 8).map((c) => {
    const target = exact(c.to);
    const call = ecosystem
      ? `; check with upgrade_impact ${JSON.stringify({ package: c.name, ...(target ? { to_version: target } : {}), ecosystem })}`
      : "";
    return `- ${c.name}: ${c.from ?? "(added)"} -> ${c.to}${call}`;
  });
  const more = changes.length > 8 ? `\n- ...and ${changes.length - 8} more` : "";
  const tail = ecosystem
    ? "Before building, call the 4da MCP tool upgrade_impact for each upgraded package: it lists the breaking changes between the versions and the files in this project that use the package."
    : "Before building, call the 4da MCP tool what_should_i_know with the upgrade as the task, and vulnerability_scan after installing.";
  return `4DA: this edit changes dependency versions in ${file}:\n${lines.join("\n")}${more}\n${tail}`;
}

async function main() {
  let raw = "";
  for await (const chunk of process.stdin) raw += chunk;
  let input;
  try {
    input = JSON.parse(raw);
  } catch {
    return;
  }
  const toolInput = input?.tool_input ?? {};
  const file = basename(String(toolInput.file_path ?? ""));
  if (!MANIFESTS.has(file)) return;

  let changes = [];
  if (input.tool_name === "Edit") {
    changes = versionChanges(file, toolInput.old_string ?? "", toolInput.new_string ?? "");
  } else if (input.tool_name === "MultiEdit" && Array.isArray(toolInput.edits)) {
    changes = toolInput.edits.flatMap((e) => versionChanges(file, e?.old_string ?? "", e?.new_string ?? ""));
  }
  // Write replaces the whole file and its payload carries no "before": stay quiet
  // rather than report every dependency as changed.
  if (changes.length === 0) return;

  process.stdout.write(
    JSON.stringify({ hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: adviceFor(file, changes) } }),
  );
}

main().catch(() => {
  // A hook must never fail the edit it observes.
});
