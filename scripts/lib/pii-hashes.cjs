/**
 * pii-hashes.cjs — single source of truth for PII detection by SHA-256 hash.
 *
 * Required by BOTH enforcement scripts:
 *   - scripts/check-doc-location.cjs   (pre-commit gate)
 *   - scripts/public-readiness-audit.cjs (on-demand audit)
 *
 * Previously this logic was copy-pasted into both files, and
 * .claude/rules/document-hygiene.md rule 5 instructed maintainers to update
 * both by hand — a drift hazard. Add a new pattern HERE and both gates pick it
 * up automatically.
 *
 * ---------------------------------------------------------------------------
 * The literal PII strings are NOT stored in this file. Instead, we hash each
 * token found in file content and compare against known hashes. This prevents
 * the enforcement scripts themselves from being a PII leak vector.
 *
 * To add a new pattern: compute the SHA-256 of the NORMALIZED string
 * (lowercased + trimmed) and append an entry to PII_HASHES:
 *
 *   node -e "console.log(require('node:crypto').createHash('sha256').update('SOMEONE@EXAMPLE.COM'.toLowerCase().trim()).digest('hex'))"
 * ---------------------------------------------------------------------------
 */

'use strict';

const { createHash } = require('node:crypto');

function sha256(str) {
  return createHash('sha256').update(str).digest('hex');
}

const PII_HASHES = [
  {
    label: 'Personal Gmail (operator — use a role alias like hello@4da.ai instead)',
    hash: '7add0e01d0b04131262b2e248429f7ef5b8592ba711519891568bcec59acebdc',
  },
  {
    label: 'Legacy Gmail (4dasystems — use a role alias like hello@4da.ai instead)',
    hash: '54565c6c5e17a54bac006628ee6a0409ef326d1f53fdc3172f4446b45d5f8df6',
  },
];

// Tokenize content: split on whitespace, angle brackets, quotes, parens,
// commas, colons (mailto:), and other common delimiters.
// This extracts email-like tokens so we can hash them individually.
function tokenize(content) {
  return content.split(/[\s<>"'(),;`:[\]{}|]+/).filter(Boolean);
}

function findPIIByHash(content) {
  const tokens = tokenize(content);
  const tokenHashes = new Map();
  for (const token of tokens) {
    const normalized = token.toLowerCase().trim();
    if (!normalized || normalized.length < 5) continue;
    if (!tokenHashes.has(normalized)) {
      tokenHashes.set(normalized, sha256(normalized));
    }
  }
  const hits = [];
  for (const { label, hash } of PII_HASHES) {
    for (const [, tokenHash] of tokenHashes) {
      if (tokenHash === hash) {
        hits.push(label);
        break;
      }
    }
  }
  return hits;
}

// PII exclusions — files where a historic reference might exist in comments
// explaining the hash system. These paths are NOT blocked.
const PII_EXCLUDE_PATHS = [
  'scripts/check-doc-location.cjs',
  'scripts/public-readiness-audit.cjs',
  'scripts/doc-allowlist.json',
];

module.exports = { sha256, PII_HASHES, tokenize, findPIIByHash, PII_EXCLUDE_PATHS };
