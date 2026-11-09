// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Prompts 4DA hands to the user's own coding agent (Claude Code, VS Code,
 * Cursor, or anything they paste into).
 *
 * Built ONLY from structured, 4DA-owned fields: dependency names from the
 * lockfile scan, versions from the version_context citation, advisory IDs
 * matched by strict patterns, and a short project label (the folder path is
 * only used to open Claude Code there, never written into the prompt, the same
 * rule as NETWORK.md "No local paths"). Third-party free text
 * (advisory summaries, feed titles, explanations) never goes in: it could
 * carry instructions aimed at the agent. The prompt is plain text, and the
 * agent only runs it after the user reads it and presses Enter.
 */

import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';
import { shortenProjectPath } from '../utils/project-path';

/** Claude Code's documented cap on a link prompt (decoded characters). */
export const HANDOFF_PROMPT_MAX_CHARS = 5000;

export const MCP_SETUP_LINE =
  'If the 4DA tools are not available here, tell me to run: claude mcp add 4da -- npx -y @4da/mcp-server';

const MAX_PACKAGES = 10;
const MAX_ADVISORIES = 12;

/** npm (incl. @scope/name), crates, PyPI, Go module paths, Ruby, PHP vendor/name. */
const PACKAGE_NAME = /^@?[A-Za-z0-9][A-Za-z0-9._~/@:+-]{0,213}$/;
const VERSION = /^v?[0-9][0-9A-Za-z.+_-]{0,63}$/;
const ADVISORY_ID =
  /\b(GHSA(?:-[23456789cfghjmpqrvwx]{4}){3}|CVE-\d{4}-\d{4,7}|RUSTSEC-\d{4}-\d{4}|PYSEC-\d{4}-\d{1,6}|GO-\d{4}-\d{4,6})\b/gi;
/** "Installed: 1.2.3 -> update to >= 1.2.4 (direct) [dev]" (preemption.rs). */
const INSTALLED_LINE =
  /^Installed: (\S+?)(?: \u2192 update to >= (\S+?))?(?: \((direct|transitive)\))?(?: \[dev\])?$/;

export interface FixFacts {
  packages: string[];
  installed: string | null;
  fixedIn: string | null;
  direct: boolean | null;
  advisories: string[];
  project: string | null;
}

/** Characters that can hide or reorder text for the person reviewing the prompt. */
function hasHiddenChars(s: string): boolean {
  // eslint-disable-next-line no-control-regex
  return /[\u0000-\u0008\u000B-\u001F\u007F-\u009F\u200B-\u200F\u202A-\u202E\u2066-\u2069\uFEFF]/.test(s);
}

function isAbsolutePath(p: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(p) || p.startsWith('/');
}

/** Pull the facts a fix prompt may carry out of an evidence item. */
export function extractFixFacts(item: EvidenceItem): FixFacts {
  const packages = item.affected_deps
    .map(d => d.trim())
    .filter(d => PACKAGE_NAME.test(d))
    .slice(0, MAX_PACKAGES);

  let installed: string | null = null;
  let fixedIn: string | null = null;
  let direct: boolean | null = null;
  const versionCite = item.evidence.find(e => e.source === 'version_context');
  const m = versionCite ? INSTALLED_LINE.exec(versionCite.title.trim()) : null;
  if (m) {
    installed = m[1] && VERSION.test(m[1]) ? m[1] : null;
    fixedIn = m[2] && VERSION.test(m[2]) ? m[2] : null;
    direct = m[3] === 'direct' ? true : m[3] === 'transitive' ? false : null;
  }

  // Advisory IDs only: the pattern match is all that survives from citation text.
  const advisories: string[] = [];
  for (const cite of item.evidence) {
    for (const text of [cite.url ?? '', cite.title]) {
      for (const hit of text.matchAll(ADVISORY_ID)) {
        const raw = hit[1];
        if (!raw) continue;
        // Canonical forms: GHSA-xxxx-xxxx-xxxx (lower-case body), CVE-.../RUSTSEC-... upper-case.
        const id = /^ghsa/i.test(raw) ? `GHSA${raw.slice(4).toLowerCase()}` : raw.toUpperCase();
        if (!advisories.includes(id)) advisories.push(id);
      }
    }
  }

  const project =
    item.affected_projects.find(p => isAbsolutePath(p) && !hasHiddenChars(p) && !p.includes('..')) ?? null;

  return {
    packages,
    installed,
    fixedIn,
    direct,
    advisories: advisories.slice(0, MAX_ADVISORIES),
    project,
  };
}

/** True when the item names at least one dependency a fix prompt can target. */
export function canBuildFixPrompt(item: EvidenceItem): boolean {
  return extractFixFacts(item).packages.length > 0;
}

/** Hand-off prompt for one dependency finding. */
export function buildFixPrompt(item: EvidenceItem): string {
  const f = extractFixFacts(item);
  const lines: string[] = ['Use the 4DA MCP tools to fix this dependency finding in this project.', ''];
  lines.push(`${f.packages.length > 1 ? 'Packages' : 'Package'}: ${f.packages.join(', ')}`);
  if (f.installed) lines.push(`Installed: ${f.installed}${f.direct === false ? ' (transitive)' : f.direct ? ' (direct)' : ''}`);
  if (f.fixedIn) lines.push(`4DA says it is fixed from: ${f.fixedIn}`);
  if (f.advisories.length > 0) lines.push(`Advisories: ${f.advisories.join(', ')}`);
  if (f.project) lines.push(`Project: ${shortenProjectPath(f.project)}`);
  lines.push(
    '',
    'Steps:',
    '1. Call dependency_check on the fix version, passing from (the installed version) and to (the target). Apply it only if the verdict is proceed.',
    '2. If it is a major upgrade (a 0.x minor counts), call upgrade_impact first and make the code changes it lists.',
    '3. Apply it with the project\'s own package manager so the lockfile updates. If the package is not a direct dependency, update it within the existing ranges or upgrade the parent that pulls it in. Never add it as a new direct dependency and never edit the lockfile by hand.',
    '4. Run the project\'s tests (and its build or type-check if it has one). If anything fails, revert the change and tell me why.',
    '5. Report what you changed, the test results, and anything you did not apply.',
    '',
    MCP_SETUP_LINE,
  );
  return capPrompt(lines.join('\n'));
}

/** Hand-off prompt for the Brief: the agent writes it from 4DA's facts. */
export function buildBriefPrompt(): string {
  return [
    'Use the 4DA tools to brief me on my dependencies across all my projects.',
    '',
    'Call upgrade_planner with project_path "*" (it runs the vulnerability scan itself). Then tell me what changed and exactly what to run, most urgent first. For each item give the package, the installed version, the version that fixes it, the projects that hold it, and the command to run.',
    'If the plan says NOT CHECKED, tell me which dependencies could not be checked. Change nothing until I say so.',
    '',
    MCP_SETUP_LINE,
  ].join('\n');
}

/** Keep a prompt under Claude Code's link cap without cutting the setup line. */
function capPrompt(prompt: string): string {
  if (prompt.length <= HANDOFF_PROMPT_MAX_CHARS) return prompt;
  const tail = `\n\n${MCP_SETUP_LINE}`;
  return prompt.slice(0, HANDOFF_PROMPT_MAX_CHARS - tail.length) + tail;
}
