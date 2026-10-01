// SPDX-License-Identifier: Apache-2.0
/**
 * MCP prompts — user-invoked workflows (e.g. `/deps` in hosts that surface
 * prompts as slash commands). A prompt is text handed to the agent; it does
 * nothing on its own and never runs without the user choosing it.
 */

export interface PromptDefinition {
  name: string;
  title: string;
  description: string;
  arguments: Array<{ name: string; description: string; required: boolean }>;
}

const DEPS_PROMPT: PromptDefinition = {
  name: "deps",
  title: "Safe dependency updates",
  description:
    "Plan dependency upgrades, vet every bump with dependency_check, apply only the safe ones in small tested batches, and report the rest with evidence.",
  arguments: [
    {
      name: "scope",
      description: "Optional focus, e.g. a package name, \"security only\", or \"patch and minor only\".",
      required: false,
    },
  ],
};

export const PROMPTS: PromptDefinition[] = [DEPS_PROMPT];

export function listPrompts(): PromptDefinition[] {
  return PROMPTS;
}

/** The prompt's messages, or null for an unknown prompt name. */
export function getPrompt(
  name: string,
  args: Record<string, string> | undefined,
): { description: string; messages: Array<{ role: "user"; content: { type: "text"; text: string } }> } | null {
  if (name !== DEPS_PROMPT.name) return null;
  const scope = args?.scope?.trim();
  return {
    description: DEPS_PROMPT.description,
    messages: [{ role: "user", content: { type: "text", text: depsWorkflow(scope) } }],
  };
}

function depsWorkflow(scope: string | undefined): string {
  return [
    "Update this project's dependencies safely, using the 4DA tools in this order.",
    scope ? `Scope for this run: ${scope}. Leave everything outside it alone.` : "",
    "",
    "1. Call `upgrade_planner` to get the proposed upgrades (run `vulnerability_scan` first if the plan says it is CVE-blind).",
    "2. Call `dependency_check` on every proposed bump, passing `from` (the installed version) and `to` (the proposed version). Batch up to 25 items per call.",
    "3. Apply ONLY items whose verdict is `proceed`. Apply them in small batches (a few packages at a time, never majors mixed with patches), using the project's own package manager so the lockfile is updated.",
    "4. After each batch, run the project's own tests (and its build or type-check if it has one). If anything fails, revert that batch, note which package caused it, and continue with the next batch.",
    "5. Do NOT apply `review`, `avoid`, `wait` or `unknown` items. Stop on them and report each one to me with its verdict, reason, and the evidence from its signals, so I can decide. For `wait`, say when the release becomes old enough.",
    "6. When done, call `vulnerability_scan` again and report: what was applied, what was skipped and why, test results per batch, and any advisories still open.",
    "",
    "Never bypass a failing test, never edit a lockfile by hand, and never apply an item whose verdict you did not get from dependency_check.",
  ]
    .filter((line, i, all) => line !== "" || all[i - 1] !== "")
    .join("\n");
}
