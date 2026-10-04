// Cloudflare Pages Function: licence-signing health check.
//
// GET /api/license/health -> 200 when the live LICENSE_PRIVATE_KEY_HEX derives the
// exact public key the desktop app verifies against, 503 otherwise. Returns only
// public material (the derived public key) — never the seed. See
// lib/license-health.js for why "the secret exists" is not enough.

import * as ed from '@noble/ed25519';
import { hexToBytes } from '../../../lib/ed25519-license.js'; // also wires @noble's SHA-512
import { signingKeyReport } from '../../../lib/license-health.js';

function toHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function json(body, status) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

export async function onRequest({ request, env }) {
  if (request.method !== 'GET') return json({ error: 'Method not allowed' }, 405);

  const seedHex = env.LICENSE_PRIVATE_KEY_HEX;
  let report;
  if (!seedHex) {
    report = signingKeyReport({ configured: false });
  } else {
    try {
      const pub = await ed.getPublicKeyAsync(hexToBytes(seedHex));
      report = signingKeyReport({ configured: true, derivedPublicKeyHex: toHex(pub) });
    } catch {
      // Never echo the exception: a parse error message could quote the input.
      report = signingKeyReport({ configured: true, error: 'seed could not be parsed' });
    }
  }
  return json(report, report.ok ? 200 : 503);
}
