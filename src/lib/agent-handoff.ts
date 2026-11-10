// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Prompts 4DA hands to the user's own coding agent (Claude Code, VS Code,
 * Cursor, or anything they paste into).
 *
 * Built ONLY from structured, 4DA-owned fields: dependency names from the
 * lockfile scan, versions from the version_context or fix-target citations,
 * parent packages from dependency-path citations (every piece re-validated by
 * a strict pattern), advisory IDs matched by strict patterns, and a short
 * project label (the folder path is
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
/** Upgrade Plan step, one per installed version: "3.4.1 -> 3.9.0" (upgrade_plan.rs). */
const FIX_TARGET_TITLE = /^(\S+) -> (\S+)$/;
/** Its note lists the sites: "... Installed in: d:/a (direct), d:/b (transitive, dev)". */
const FIX_TARGET_SITE = /([^,]+?) \((direct|transitive)(?:, dev)?\)(?:, |$)/g;
/** "rmcp 3.4.1 <- victauri-plugin 0.10.3" (upgrade_parent.rs). */
const DEP_PATH_TITLE = /^(\S+) (\S+) <- (\S+) (\S+)$/;
/**
 * "In d:/a; victauri-plugin is a direct dependency; requires ^3.1, which admits 3.9.0", ending
 * "; blocked upstream" when no published release of the parent (newest included) admits the fix
 * (upgrade_parent.rs `link_note`, Y5).
 */
const DEP_PATH_NOTE =
  /^In (.+?); \S+ is a (?:direct|transitive) dependency; requires (.+?), which (admits|excludes) (\S+?)(?:; blocked upstream)?$/;
const BLOCKED_SUFFIX = /; blocked upstream$/;
/** One comparator of a version requirement: "^3.1", ">=1.0.0-rc.1", "~=3.1", "1.x", "*". */
const REQUIREMENT_TOKEN = /^[\^~=<>!]*[0-9*xX][0-9A-Za-z.+_*-]*$/;
const MAX_PARENTS = 3;

/** A version requirement as lockfiles record it ("^3.1", ">=1.2, <2", "1.x || 2.x"); no words. */
function isRequirement(s: string): boolean {
  const tokens = s.split(/[\s,|]+/).filter(Boolean);
  return s.length <= 40 && tokens.length > 0 && tokens.every(tok => REQUIREMENT_TOKEN.test(tok));
}

export interface FixFacts {
  packages: string[];
  installed: string | null;
  fixedIn: string | null;
  direct: boolean | null;
  /** Parent packages that pull the dependency into the project, with what their requirement allows. */
  parents: string[];
  advisories: string[];
  project: string | null;
}

/** Folder paths compare case- and separator-insensitively (the scan lower-cases them on Windows). */
function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.trim().replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
  return norm(a) === norm(b);
}

/** Installed and fix version for `project` from Upgrade Plan fix-target citations. */
function fixTargetFor(
  item: EvidenceItem,
  project: string | null,
): { installed: string | null; fixedIn: string | null; direct: boolean | null } {
  const targets: string[] = [];
  for (const cite of item.evidence) {
    if (cite.source !== 'fix-target') continue;
    const m = FIX_TARGET_TITLE.exec(cite.title.trim());
    if (!m?.[1] || !m[2] || !VERSION.test(m[1]) || !VERSION.test(m[2])) continue;
    targets.push(m[2]);
    const sites = (cite.relevance_note ?? '').split('Installed in: ')[1] ?? '';
    for (const site of sites.matchAll(FIX_TARGET_SITE)) {
      if (project && site[1] && samePath(site[1], project)) {
        return { installed: m[1], fixedIn: m[2], direct: site[2] === 'direct' };
      }
    }
  }
  // No site matched: still name the fix when every installed version shares it.
  const unique = [...new Set(targets)];
  return { installed: null, fixedIn: unique.length === 1 ? (unique[0] ?? null) : null, direct: null };
}

/** "victauri-plugin 0.10.3, which requires ^3.1 (admits 3.9.0: a lockfile refresh is enough)". */
function parentsFor(item: EvidenceItem, packages: string[], project: string | null): string[] {
  const out: string[] = [];
  for (const cite of item.evidence) {
    if (cite.source !== 'dependency-path' || out.length >= MAX_PARENTS) continue;
    const t = DEP_PATH_TITLE.exec(cite.title.trim());
    if (!t?.[1] || !t[2] || !t[3] || !t[4]) continue;
    const [, child, childVersion, parent, parentVersion] = t;
    if (!packages.includes(child) || !VERSION.test(childVersion) || !PACKAGE_NAME.test(parent) || !VERSION.test(parentVersion)) continue;
    // Every link note starts "In <project>; " — a link in another project is not this fix.
    const note = (cite.relevance_note ?? '').trim();
    const inProject = /^In (.+?); /.exec(note)?.[1];
    if (!inProject || !project || !samePath(inProject, project)) continue;
    const n = DEP_PATH_NOTE.exec(note);
    const blocked = BLOCKED_SUFFIX.test(note);
    let line = `${parent} ${parentVersion} (pulls in ${child} ${childVersion})`;
    if (n?.[2] && n[4] && isRequirement(n[2]) && VERSION.test(n[4])) {
      if (n[3] === 'admits') {
        line += `, which requires ${n[2]}: that admits ${n[4]}, so refreshing the lockfile is enough`;
      } else if (blocked) {
        line += `, which requires ${n[2]}: that excludes ${n[4]}, and no published ${parent} release allows it yet (blocked upstream), so neither a lockfile refresh nor a ${parent} upgrade can fix it`;
      } else {
        line += `, which requires ${n[2]}: that excludes ${n[4]}, so a lockfile refresh cannot reach it until a ${parent} release allows it`;
      }
    } else if (blocked) {
      line += `: no published ${parent} release lets it reach the fix yet (blocked upstream)`;
    }
    if (!out.includes(line)) out.push(line);
  }
  return out;
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

  // Upgrade Plan steps carry their versions as fix-target citations instead.
  if (!m) ({ installed, fixedIn, direct } = fixTargetFor(item, project));

  return {
    packages,
    installed,
    fixedIn,
    direct,
    parents: parentsFor(item, packages, project),
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
  for (const parent of f.parents) lines.push(`Pulled in by: ${parent}`);
  if (f.advisories.length > 0) lines.push(`Advisories: ${f.advisories.join(', ')}`);
  if (f.project) lines.push(`Project: ${shortenProjectPath(f.project)}`);
  const blocked = f.parents.some(p => p.includes('(blocked upstream)'));
  lines.push(
    '',
    'Steps:',
    '1. Call dependency_check on the fix version, passing from (the installed version) and to (the target). Apply it only if the verdict is proceed.',
    '2. If it is a major upgrade (a 0.x minor counts), call upgrade_impact first and make the code changes it lists.',
    '3. Apply it with the project\'s own package manager so the lockfile updates. If the package is not a direct dependency, update it within the existing ranges or upgrade the parent that pulls it in. Never add it as a new direct dependency and never edit the lockfile by hand.',
    ...(blocked
      ? ['   A parent marked blocked upstream has no release that fixes this yet: do not upgrade or refresh for it. Tell me the options instead (an override or `[patch]` pointing at a fork, a fork of the parent, or accepting the risk with a review date) and change nothing on that path until I choose.']
      : []),
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
