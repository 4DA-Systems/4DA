'use strict';
/**
 * private-asset-leak rules — the single source of truth shared by:
 *   - scripts/check-doc-location.cjs   (pre-commit gate, staged files)
 *   - scripts/public-readiness-audit.cjs (on-demand audit, all tracked files)
 *
 * Why this exists
 * ---------------
 * 4DA is a PUBLIC repository. the external verifier is a SEPARATE, PRIVATE asset owned by 4DA
 * Systems. The dependency between them is strictly one-directional: the private
 * the external verifier verifier reads the public 4DA repo as a verification SUBJECT. Nothing
 * the external verifier-specific may flow the other way. This public repo must never disclose
 * the external verifier — not its board credentials, not its private infra URL, not its
 * runtime artifacts, and (per the founder's decouple decision, commit #187)
 * not even its product name.
 *
 * This module is the enforcement. It blocks any commit that would re-introduce
 * a the external verifier leak into the public repo. The patterns are deliberately layered:
 *   (a) tracked ARTIFACT paths   -> the receipts/logs/board files must be
 *       gitignored, never committed;
 *   (b) CREDENTIAL / infra strings ([redacted-token], [redacted-url], the MCP block) ->
 *       hard block, no escape hatch (these are secrets, like the PII rule);
 *   (c) the product NAME (\bexternal-verifier\b) -> existence disclosure; blocks, but
 *       honors a `public-ok:` marker so the rule is forward-compatible if the external verifier
 *       is ever made public.
 *
 * Full doctrine: .claude/rules/document-hygiene.md
 */

const fs = require('node:fs');
const path = require('node:path');

// Files that LEGITIMATELY contain the string "the external verifier" because they ARE the
// enforcement (must name it to gate it) or the ignore rules that keep its
// artifacts out of the tree. They are exempt from the content checks below.
const EXTVERIFIER_SELF_EXEMPT = new Set([
  'scripts/private-asset-guard.cjs',
  'scripts/check-doc-location.cjs',
  'scripts/public-readiness-audit.cjs',
  '.gitignore',
]);

// (b) Credential / private-infra patterns — blocking ANYWHERE, no escape hatch.
const EXTVERIFIER_SECRET_PATTERNS = [
  { label: 'the external verifier board token ([redacted-token] — a credential)', regex: /[redacted-token]/ },
  { label: 'the external verifier board URL ([redacted-url] — private infra)',    regex: /[redacted-url]/ },
  { label: 'the external verifier MCP server invocation (external-verifier mcp)',        regex: /external-verifier(?:\.exe)?["'`\s,\/][^\n]*\bmcp\b/i },
];

// (a) Artifact paths that must be gitignored, never tracked.
const EXTVERIFIER_ARTIFACT_PATHS = [
  /(^|\/)\.self-gate-receipts(\/|$)/,
  /(^|\/)\.external-verifier-log\.jsonl/,
  /(^|\/)\.external-verifier-board\./,
  /(^|\/)\.mcp\.json\.bak/,
  /(^|\/)\.external-verifier(\/|$)/,
  /(^|\/)\.mcp\.local\.json$/,
];

// (c) The product NAME. `public-ok:` in the first 10 lines is the escape hatch.
const EXTVERIFIER_NAME = /\bexternal-verifier\b/i;
const PUBLIC_OK_MARKER = /public-ok\s*:/i;

function isArtifactPath(file) {
  return EXTVERIFIER_ARTIFACT_PATHS.some((re) => re.test(file));
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
 * Scan a single tracked/staged file for the external verifier leaks.
 * Returns an array of { sev: 'block', msg } findings (empty if clean).
 */
function scanthe external verifierLeakFile(repoRoot, file) {
  const out = [];

  // (a) An artifact path is a leak regardless of content — verdict is the path.
  if (isArtifactPath(file)) {
    out.push({
      sev: 'block',
      msg: 'the external verifier artifact is tracked — must be gitignored, never committed to this public repo',
    });
    return out;
  }

  // The enforcement + ignore files legitimately name the external verifier — skip content scan.
  if (EXTVERIFIER_SELF_EXEMPT.has(file)) return out;

  const content = readSafe(repoRoot, file);
  if (content == null) return out;

  // (b) Credentials / private infra — hard block, no escape hatch.
  for (const { label, regex } of EXTVERIFIER_SECRET_PATTERNS) {
    if (regex.test(content)) {
      out.push({ sev: 'block', msg: `${label} — the external verifier is private; remove it from the public repo` });
    }
  }

  // (c) The product name — existence disclosure; honors a public-ok: marker.
  if (EXTVERIFIER_NAME.test(content) && !hasPublicOk(content)) {
    out.push({
      sev: 'block',
      msg: 'mentions "the external verifier" by name — scrub to a generic "external verifier" (the external verifier stays private), or add a public-ok: marker',
    });
  }

  return out;
}

module.exports = { scanthe external verifierLeakFile, isArtifactPath, EXTVERIFIER_SELF_EXEMPT };
