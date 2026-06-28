'use strict';
/**
 * Verax-leak rules — the single source of truth shared by:
 *   - scripts/check-doc-location.cjs   (pre-commit gate, staged files)
 *   - scripts/public-readiness-audit.cjs (on-demand audit, all tracked files)
 *
 * Why this exists
 * ---------------
 * 4DA is a PUBLIC repository. Verax is a SEPARATE, PRIVATE asset owned by 4DA
 * Systems. The dependency between them is strictly one-directional: the private
 * Verax verifier reads the public 4DA repo as a verification SUBJECT. Nothing
 * Verax-specific may flow the other way. This public repo must never disclose
 * Verax — not its board credentials, not its private infra URL, not its
 * runtime artifacts, and (per the founder's decouple decision, commit #187)
 * not even its product name.
 *
 * This module is the enforcement. It blocks any commit that would re-introduce
 * a Verax leak into the public repo. The patterns are deliberately layered:
 *   (a) tracked ARTIFACT paths   -> the receipts/logs/board files must be
 *       gitignored, never committed;
 *   (b) CREDENTIAL / infra strings (VERAX_TOKEN, VERAX_URL, the MCP block) ->
 *       hard block, no escape hatch (these are secrets, like the PII rule);
 *   (c) the product NAME (\bverax\b) -> existence disclosure; blocks, but
 *       honors a `public-ok:` marker so the rule is forward-compatible if Verax
 *       is ever made public.
 *
 * Full doctrine: .claude/rules/document-hygiene.md
 */

const fs = require('node:fs');
const path = require('node:path');

// Files that LEGITIMATELY contain the string "Verax" because they ARE the
// enforcement (must name it to gate it) or the ignore rules that keep its
// artifacts out of the tree. They are exempt from the content checks below.
const VERAX_SELF_EXEMPT = new Set([
  'scripts/verax-leak-rules.cjs',
  'scripts/check-doc-location.cjs',
  'scripts/public-readiness-audit.cjs',
  '.gitignore',
]);

// (b) Credential / private-infra patterns — blocking ANYWHERE, no escape hatch.
const VERAX_SECRET_PATTERNS = [
  { label: 'Verax board token (VERAX_TOKEN — a credential)', regex: /VERAX_TOKEN/ },
  { label: 'Verax board URL (VERAX_URL — private infra)',    regex: /VERAX_URL/ },
  { label: 'Verax MCP server invocation (verax mcp)',        regex: /verax(?:\.exe)?["'`\s,\/][^\n]*\bmcp\b/i },
];

// (a) Artifact paths that must be gitignored, never tracked.
const VERAX_ARTIFACT_PATHS = [
  /(^|\/)\.self-gate-receipts(\/|$)/,
  /(^|\/)\.verax-log\.jsonl/,
  /(^|\/)\.verax-board\./,
  /(^|\/)\.mcp\.json\.bak/,
  /(^|\/)\.verax(\/|$)/,
  /(^|\/)\.mcp\.local\.json$/,
];

// (c) The product NAME. `public-ok:` in the first 10 lines is the escape hatch.
const VERAX_NAME = /\bverax\b/i;
const PUBLIC_OK_MARKER = /public-ok\s*:/i;

function isArtifactPath(file) {
  return VERAX_ARTIFACT_PATHS.some((re) => re.test(file));
}

function hasPublicOk(content) {
  return PUBLIC_OK_MARKER.test(content.split('\n').slice(0, 10).join('\n'));
}

function readSafe(repoRoot, file) {
  try {
    const abs = path.join(repoRoot, file);
    const stat = fs.statSync(abs);
    if (stat.size > 2 * 1024 * 1024) return null; // > 2 MB: skip
    const buf = fs.readFileSync(abs);
    if (buf.includes(0)) return null; // binary: contains a NUL byte
    return buf.toString('utf8');
  } catch {
    return null;
  }
}

/**
 * Scan a single tracked/staged file for Verax leaks.
 * Returns an array of { sev: 'block', msg } findings (empty if clean).
 */
function scanVeraxLeakFile(repoRoot, file) {
  const out = [];

  // (a) An artifact path is a leak regardless of content — verdict is the path.
  if (isArtifactPath(file)) {
    out.push({
      sev: 'block',
      msg: 'Verax artifact is tracked — must be gitignored, never committed to this public repo',
    });
    return out;
  }

  // The enforcement + ignore files legitimately name Verax — skip content scan.
  if (VERAX_SELF_EXEMPT.has(file)) return out;

  const content = readSafe(repoRoot, file);
  if (content == null) return out;

  // (b) Credentials / private infra — hard block, no escape hatch.
  for (const { label, regex } of VERAX_SECRET_PATTERNS) {
    if (regex.test(content)) {
      out.push({ sev: 'block', msg: `${label} — Verax is private; remove it from the public repo` });
    }
  }

  // (c) The product name — existence disclosure; honors a public-ok: marker.
  if (VERAX_NAME.test(content) && !hasPublicOk(content)) {
    out.push({
      sev: 'block',
      msg: 'mentions "Verax" by name — scrub to a generic "external verifier" (Verax stays private), or add a public-ok: marker',
    });
  }

  return out;
}

module.exports = { scanVeraxLeakFile, isArtifactPath, VERAX_SELF_EXEMPT };
