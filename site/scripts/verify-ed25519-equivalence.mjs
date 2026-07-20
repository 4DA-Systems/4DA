// Proof that the new @noble/ed25519 signing path produces BYTE-IDENTICAL license
// signatures to the old Node `crypto` path used by the Vercel handler.
//
// Both implement RFC 8032 "pure" Ed25519, which is deterministic: the same 32-byte
// seed + same message => the same 64-byte signature. The desktop app's Rust verifier
// (ed25519_dalek) already accepts the Node output, so byte-equality proves it will
// accept the @noble output too. Run: `node scripts/verify-ed25519-equivalence.mjs`
// (or `pnpm run verify:ed25519`). Exit 0 = pass.

import crypto from 'node:crypto';
import * as ed from '@noble/ed25519';

// Wire @noble's async SHA-512 to WebCrypto — exactly as functions/api/streets/activate.js does.
ed.etc.sha512Async = async (...msgs) => {
  let total = 0;
  for (const m of msgs) total += m.length;
  const data = new Uint8Array(total);
  let offset = 0;
  for (const m of msgs) {
    data.set(m, offset);
    offset += m.length;
  }
  return new Uint8Array(await crypto.webcrypto.subtle.digest('SHA-512', data));
};

// Fixed 32-byte seed (test only — NOT a real key).
const privHex = '9d61b19deffdaa4b27b0e9d4a5c9f3f28e0dd3a9f5c9f0e1d2a3b4c5d6e7f8091';
const seedBytes = Buffer.from(privHex, 'hex');
if (seedBytes.length !== 32) throw new Error(`bad seed length ${seedBytes.length}`);

// A representative license payload (same shape activate.js signs).
const payload = {
  tier: 'signal',
  email: 'test@example.com',
  expires_at: '2027-01-01T00:00:00.000Z',
  issued_at: '2026-01-01T00:00:00.000Z',
  features: ['signal'],
};
const payloadBytes = Buffer.from(JSON.stringify(payload), 'utf8');

// --- OLD path: Node crypto (verbatim from the Vercel activate.js) ---
const keyObject = crypto.createPrivateKey({
  key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seedBytes]),
  format: 'der',
  type: 'pkcs8',
});
const nodeSig = crypto.sign(null, payloadBytes, keyObject);

// --- NEW path: @noble/ed25519 (as in activate.js) ---
const nobleSig = Buffer.from(await ed.signAsync(new Uint8Array(payloadBytes), new Uint8Array(seedBytes)));

console.log('node  sig (b64):', nodeSig.toString('base64'));
console.log('noble sig (b64):', nobleSig.toString('base64'));

// Cross-check: Node verifies the noble signature (i.e. an independent verifier accepts it).
const pubKey = crypto.createPublicKey(keyObject);
const nodeAcceptsNoble = crypto.verify(null, payloadBytes, pubKey, nobleSig);

if (nodeSig.length === 64 && nobleSig.length === 64 && Buffer.compare(nodeSig, nobleSig) === 0 && nodeAcceptsNoble) {
  console.log('PASS: signatures byte-identical AND Node verifier accepts the @noble signature.');
  process.exit(0);
} else {
  console.error('FAIL: byteEqual=%s nodeAcceptsNoble=%s', Buffer.compare(nodeSig, nobleSig) === 0, nodeAcceptsNoble);
  process.exit(1);
}
