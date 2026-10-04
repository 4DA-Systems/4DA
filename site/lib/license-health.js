// Does the live licence-signing key match the key the desktop app trusts?
//
// WHY THIS EXISTS
// ---------------
// Every Signal key is signed server-side with LICENSE_PRIVATE_KEY_HEX and verified
// OFFLINE by the app against a public key compiled into the binary
// (src-tauri/src/settings/license/verify.rs). If the Cloudflare secret ever holds a
// different seed — a mistyped paste, a rotation done on one side only, a migration
// that copied the wrong value — every purchase still succeeds, every webhook still
// answers 200, every key is still emailed, and every key is then rejected by the
// app with "signature verification failed". Nothing in the payment path can see it.
//
// Cloudflare secrets are write-only, so scripts/check-pages-secrets.cjs can prove
// the secret EXISTS but never that it is CORRECT. The public key derived from the
// seed is not secret (it ships inside every installer), so the live endpoint can
// derive it and compare — proving correctness while disclosing nothing.
//
// DELIBERATELY DEPENDENCY-FREE (no @noble import): the derivation happens in the
// Pages Function; this module only judges the result, so test:scripts can run it.

/**
 * The public key the shipped app verifies against. MUST equal
 * LICENSE_PUBLIC_KEY_HEX in src-tauri/src/settings/license/verify.rs — pinned by
 * license-health.test.mjs, which reads that file.
 */
export const APP_LICENSE_PUBLIC_KEY_HEX =
  '084dc1b1b9549bf0ddff11db9186cb623ceb9d72831fbf2e6f01db160388f9d6';

/**
 * Build the public health report.
 *
 * @param {{configured: boolean, derivedPublicKeyHex?: string|null, error?: string|null}} input
 * @returns {{ok: boolean, signing_key: object}}
 */
export function signingKeyReport({ configured, derivedPublicKeyHex = null, error = null }) {
  const derived = typeof derivedPublicKeyHex === 'string' ? derivedPublicKeyHex.toLowerCase() : null;
  const matches = configured && derived === APP_LICENSE_PUBLIC_KEY_HEX;
  return {
    ok: matches,
    signing_key: {
      configured: Boolean(configured),
      matches_app: matches,
      // Public by construction: the same bytes are in every installer.
      expected_public_key: APP_LICENSE_PUBLIC_KEY_HEX,
      derived_public_key: derived,
      ...(error ? { error } : {}),
    },
  };
}
