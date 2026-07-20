// Shared Ed25519 license-token signing for Cloudflare Pages Functions.
//
// Produces the app's canonical `4DA-{base64(payload)}.{base64(sig)}` token, which
// the desktop app verifies OFFLINE against its embedded public key
// (src-tauri/src/settings/license/verify.rs). The private key seed is the
// LICENSE_PRIVATE_KEY_HEX secret (matches embedded pubkey 084dc1b1...).
//
// Signatures are byte-identical to Node crypto / Rust ed25519_dalek for the same
// seed+message (RFC 8032). Proven by scripts/verify-ed25519-equivalence.mjs.

import * as ed from '@noble/ed25519';

// Wire @noble's async SHA-512 to the runtime's WebCrypto (Workers + Node 20+).
ed.etc.sha512Async = async (...msgs) => {
  let total = 0;
  for (const m of msgs) total += m.length;
  const data = new Uint8Array(total);
  let offset = 0;
  for (const m of msgs) {
    data.set(m, offset);
    offset += m.length;
  }
  return new Uint8Array(await crypto.subtle.digest('SHA-512', data));
};

export function hexToBytes(hex) {
  const clean = hex.trim();
  if (clean.length % 2 !== 0) throw new Error('hex length must be even');
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substr(i * 2, 2), 16);
  }
  return bytes;
}

export function bytesToB64(bytes) {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

/**
 * Sign a license payload into a `4DA-...` token.
 * @param {object} payload  Must match the app's LicensePayload shape:
 *   { tier, email, expires_at, issued_at, features, [license_id] }.
 *   Extra fields are allowed (verifier ignores unknown fields) but the SIGNED
 *   bytes are exactly JSON.stringify(payload) — key order must stay stable.
 * @param {string} privHex  64-char hex Ed25519 seed (LICENSE_PRIVATE_KEY_HEX).
 * @returns {Promise<string>} `4DA-{b64payload}.{b64sig}`
 */
export async function signLicenseToken(payload, privHex) {
  if (!privHex) throw new Error('LICENSE_PRIVATE_KEY_HEX not configured');
  const seed = hexToBytes(privHex);
  if (seed.length !== 32) throw new Error('LICENSE_PRIVATE_KEY_HEX must be 32 bytes (64 hex chars)');

  const payloadBytes = new TextEncoder().encode(JSON.stringify(payload));
  const payloadB64 = bytesToB64(payloadBytes);
  const sig = await ed.signAsync(payloadBytes, seed); // 64-byte Uint8Array
  return `4DA-${payloadB64}.${bytesToB64(sig)}`;
}

/**
 * Generate a stable, unguessable refresh credential (the user-facing "license
 * key" in the lease model). Format: `4DA-LIC-<52 base32 chars>` (~256 bits).
 * Distinguishable from signed tokens by the `-LIC-` marker and absence of `.`.
 */
export function generateRefreshKey() {
  const raw = new Uint8Array(32);
  crypto.getRandomValues(raw);
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'; // RFC4648 base32, no padding
  let out = '';
  for (let i = 0; i < raw.length; i++) {
    out += alphabet[raw[i] & 31];
    out += alphabet[(raw[i] >> 5) & 31];
  }
  return `4DA-LIC-${out}`;
}
