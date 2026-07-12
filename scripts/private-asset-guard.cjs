'use strict';
/**
 * Private-asset leak guard — the single source of truth shared by:
 *   - scripts/check-doc-location.cjs      (pre-commit gate, staged files)
 *   - scripts/public-readiness-audit.cjs  (on-demand audit, all tracked files)
 *   - .husky/commit-msg                   (commit-message gate)
 *
 * Why this exists
 * ---------------
 * This is a PUBLIC repository. It depends on a SEPARATE, PRIVATE asset (an
 * external verifier owned by the same company). The dependency is strictly
 * one-directional: the private verifier reads this public repo as a
 * verification SUBJECT. Nothing private may flow the other way. This public
 * repo must never disclose that asset — not its credentials, not its infra
 * URL, not its runtime artifacts, and not even its product name.
 *
 * How the name stays out of THIS file
 * -----------------------------------
 * The blocked term is NOT stored here as a literal (that would itself be the
 * disclosure the guard is meant to prevent). Instead we store its SHA-256
 * hash and hash each token of scanned content to compare — the exact
 * mechanism this repo already uses for PII (see check-doc-location.cjs). A
 * delimiter-rich tokenizer means `Foo`, `FOO_TOKEN`, `.foo/`, and `.foo-log`
 * all reduce to the same bare token, so one hash covers name, credential,
 * path, and artifact forms.
 *
 * Full doctrine: .claude/rules/document-hygiene.md
 */

const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

function sha256(str) {
  return createHash('sha256').update(str).digest('hex');
}

// SHA-256 of the blocked private-asset tokens (lowercased). No literal name here.
const ASSET_TERM_HASHES = new Set([
  '3b01fbd96a4eb7b550f483723ce5db19970af945b91930a224f78717a7023ad2',
  '344ceb6cff3675fc2ce7e5f83158d83ca9d8f45567f6313d8b9ae34aefdaae01',
]);

// Split content/paths into candidate tokens. Rich delimiter set so name,
// credential (FOO_TOKEN), path (.foo/gate.json), and artifact (.foo-log.jsonl)
// forms all yield the bare token.
function tokenize(text) {
  return String(text).split(/[\s<>"'`(),._/:=\[\]{}#*!?\\|@&$-]+/);
}

function contentHasAsset(text) {
  for (const raw of tokenize(text)) {
    const t = raw.toLowerCase().trim();
    if (t.length < 4) continue;
    if (ASSET_TERM_HASHES.has(sha256(t))) return true;
  }
  return false;
}

const PUBLIC_OK_MARKER = /public-ok\s*:/i;
function hasPublicOk(content) {
  return PUBLIC_OK_MARKER.test(content.split('\n').slice(0, 10).join('\n'));
}

// A path whose own segments hash to a blocked term is a leak regardless of
// content (e.g. a tracked private-asset directory or artifact log).
function isArtifactPath(file) {
  return contentHasAsset(file);
}

function readSafe(repoRoot, file) {
  try {
    const abs = path.join(repoRoot, file);
    const stat = fs.statSync(abs);
    if (stat.size > 2 * 1024 * 1024) return null; // > 2 MB: skip
    const buf = fs.readFileSync(abs);
    if (buf.includes(0)) return null; // binary
    return buf.toString('utf8');
  } catch {
    return null;
  }
}

/**
 * Scan a single tracked/staged file for private-asset leaks.
 * Returns an array of { sev: 'block', msg } findings (empty if clean).
 */
function scanPrivateAssetLeakFile(repoRoot, file) {
  const out = [];

  // A path that names the private asset is a leak regardless of content.
  if (isArtifactPath(file)) {
    out.push({
      sev: 'block',
      msg: 'path names the private external verifier — must be gitignored, never committed to this public repo',
    });
    return out;
  }

  const content = readSafe(repoRoot, file);
  if (content == null) return out;

  if (contentHasAsset(content) && !hasPublicOk(content)) {
    out.push({
      sev: 'block',
      msg: 'mentions the private external verifier by name/token — scrub to a generic "external verifier", or add a public-ok: marker',
    });
  }

  return out;
}

// Scan an arbitrary text blob (e.g. a commit message) — no file I/O.
function scanText(text) {
  return contentHasAsset(text);
}

module.exports = { scanPrivateAssetLeakFile, isArtifactPath, scanText, sha256 };
