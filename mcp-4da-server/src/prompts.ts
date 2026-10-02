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
    "1. Call `upgrade_planner` to get the proposed upgrades (it runs the vulnerability scan itself; if it says it is CVE-blind, call it again once the scan completes). When its provenance is `app_plan`, each step is a work order: for every installed line it gives `installed`, `target`, `upgrade_type` and the project sites, plus a `mechanism`. Only touch steps whose sites include this project.",
    "2. Call `dependency_check` on every proposed bump, passing `from` (the installed version) and `to` (the proposed version). Batch up to 25 items per call.",
    "3. For each `proceed` item that is a major upgrade (a 0.x minor counts), call `upgrade_impact` with the package and target version first: it lists the breaking changes and the files here that import the package. Make the code changes it calls for in the same batch, and give each major its own batch.",
    "4. Apply ONLY items whose verdict is `proceed`. Apply them in small batches (a few packages at a time, never majors mixed with patches), using the project's own package manager so the lockfile is updated. A `target` is a minimum: ask the package manager for at least that version and let it resolve the exact one. Follow the step's `mechanism`:",
    "   - `manifest_bump`: raise the declared version range in this project's manifest, then install.",
    "   - `lockfile_or_parent_update`: the package is not declared here. Update it within the existing ranges with the package manager's update command for that one package; if the ranges do not allow the target, update the direct dependency that pulls it in instead. Never add the transitive package as a new direct dependency.",
    "   - `mixed`: handle each site by whether it is direct or transitive, as above.",
    "   - `no_fix`: no fixed version exists yet. Do not change anything; report it.",
    "5. After each batch, run the project's own tests (and its build or type-check if it has one). If anything fails, revert that batch, note which package caused it, and continue with the next batch.",
    "6. Do NOT apply `review`, `avoid`, `wait` or `unknown` items. Stop on them and report each one to me with its verdict, reason, and the evidence from its signals, so I can decide. For `wait`, say when the release becomes old enough. If the package manager refuses a version because of its own policy (for example a minimum release age or a trust check), report that too; do not change the policy.",
    "7. When done, call `vulnerability_scan` again and report: what was applied, what was skipped and why, test results per batch, and any advisories still open. If the app plan was used, say that a 4DA rescan will drop the finished steps.",
    "",
    "Never bypass a failing test, never edit a lockfile by hand, and never apply an item whose verdict you did not get from dependency_check.",
  ]
    .filter((line, i, all) => line !== "" || all[i - 1] !== "")
    .join("\n");
}
