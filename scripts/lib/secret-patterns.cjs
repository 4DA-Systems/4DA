// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * secret-patterns.cjs — the one list of credential SHAPES the repo scans for.
 *
 * Lives here, next to pii-hashes.cjs, for the same reason: so the pre-commit
 * gate and the public-readiness audit cannot drift apart, and so the patterns
 * can be regression-tested without executing an audit.
 *
 * The character class matters more than the vendor prefix. The original
 * OpenAI pattern was `/sk-[a-zA-Z0-9]{32,}/`, which stops at the first hyphen
 * — so it matched the LEGACY bare-alphanumeric key and missed both formats the
 * app actually accepts today, `sk-ant-api03-...` and `sk-proj-...`. An audit
 * that reports "no findings" while blind to the two keys most likely to leak
 * is worse than no audit, because it is believed.
 */

const SECRET_PATTERNS = [
  // Covers sk-ant-api03-*, sk-proj-*, and the legacy bare-alphanumeric form.
  { label: 'OpenAI/Anthropic secret key', regex: /\bsk-[A-Za-z0-9_-]{20,}/ },
  { label: 'Stripe live secret key', regex: /\bsk_live_[A-Za-z0-9]{20,}/ },
  { label: 'Stripe live restricted key', regex: /\brk_live_[A-Za-z0-9]{20,}/ },
  { label: 'Slack token', regex: /\bxox[baprs]-[A-Za-z0-9-]{10,}/ },
  { label: 'GitHub token', regex: /\bgh[pousr]_[A-Za-z0-9]{30,}/ },
  { label: 'Groq API key', regex: /\bgsk_[A-Za-z0-9]{20,}/ },
  // Two-segment structure (re_<id>_<secret>) on purpose: a bare
  // re_ + 20-or-more [A-Za-z0-9_-] matched ordinary snake_case identifiers in
  // the test suite -- re_reporting_against, re_dependency_alert, re_audit_alert.
  { label: 'Resend API key', regex: /\bre_[A-Za-z0-9]{8,}_[A-Za-z0-9]{20,}/ },
  {
    label: 'DeepL auth key',
    regex: /\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:fx\b/,
  },
  { label: 'AWS access key', regex: /\bAKIA[A-Z0-9]{16}\b/ },
  { label: 'Google API key', regex: /\bAIza[a-zA-Z0-9_-]{30,}\b/ },
  {
    label: 'JWT',
    regex: /\beyJ[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\.[a-zA-Z0-9_-]{20,}\b/,
  },
  { label: 'Private key block', regex: /BEGIN (RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY/ },
];

/**
 * Markers that identify a hand-written stand-in rather than a real credential.
 * Applied to the MATCHED TEXT, never to the file path: a path-based exemption
 * would blanket-excuse a genuine key that lands in a test file, which is one of
 * the likelier ways a key leaks.
 */
const PLACEHOLDER_MARKERS =
  /(test|example|dummy|fake|placeholder|donotuse|sample|redacted|xxxx|your[-_]?key|notreal|cannot-reach|rate-limited)/i;

/**
 * Is this match a fixture rather than a leak?
 *
 * Two independent signals, either sufficient:
 *  1. an explicit placeholder marker in the matched text;
 *  2. no uppercase letter anywhere in the match. Real keys carry base62/base64url
 *     bodies of 20+ characters, so the chance of zero uppercase is negligible,
 *     while hand-written stand-ins (sk-ant-realkey-1234567890) are all lowercase.
 *
 * The residual risk is a real key that happens to contain "test" -- roughly one in
 * a hundred thousand for a 90-character body, and the tradeoff for a gate that
 * stays switched on.
 */
function looksLikePlaceholder(match) {
  if (PLACEHOLDER_MARKERS.test(match)) return true;
  return !/[A-Z]/.test(match);
}

module.exports = { SECRET_PATTERNS, looksLikePlaceholder };
