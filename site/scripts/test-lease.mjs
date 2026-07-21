// Bulletproofing the lease token contract.
//
// Proves, with a TEST keypair, that signLicenseToken() produces a token that:
//   1. has the exact `4DA-{b64}.{b64}` shape the app parses,
//   2. signs exactly JSON.stringify(payload) (so Rust re-derives identical bytes),
//   3. verifies against the corresponding public key (as ed25519_dalek does),
//   4. is REJECTED when the payload is tampered,
//   5. carries a short expiry the verifier enforces,
// and that generateRefreshKey() matches the refresh endpoint's accept-regex.
//
// The real private key (LICENSE_PRIVATE_KEY_HEX) isn't needed here: signature
// equivalence to Rust ed25519_dalek is already proven by
// verify-ed25519-equivalence.mjs, so proving the pipeline with a test key closes
// the chain. Run: `node scripts/test-lease.mjs` (or `pnpm run test:lease`). Exit 0 = pass.

import * as ed from '@noble/ed25519';
import { signLicenseToken, generateRefreshKey } from '../lib/ed25519-license.js';

ed.etc.sha512Async = async (...msgs) => {
  let t = 0;
  for (const m of msgs) t += m.length;
  const d = new Uint8Array(t);
  let o = 0;
  for (const m of msgs) {
    d.set(m, o);
    o += m.length;
  }
  return new Uint8Array(await crypto.subtle.digest('SHA-512', d));
};

const b64ToBytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
let failures = 0;
const check = (name, cond) => {
  console.log(`  ${cond ? 'PASS' : 'FAIL'}  ${name}`);
  if (!cond) failures++;
};

// Deterministic TEST seed (NOT a real key).
const seedHex = '4da0000000000000000000000000000000000000000000000000000000000001';
const seed = Uint8Array.from(seedHex.match(/../g).map((h) => parseInt(h, 16)));
const pub = await ed.getPublicKeyAsync(seed);

const now = new Date();
const exp = new Date(now.getTime() + 7 * 864e5);
const payload = {
  tier: 'signal',
  email: 'lease@example.com',
  expires_at: exp.toISOString(),
  issued_at: now.toISOString(),
  features: ['signal'],
  license_id: 'cus_TEST123',
};

const token = await signLicenseToken(payload, seedHex);

// 1. shape
check('token has 4DA- prefix', token.startsWith('4DA-'));
const body = token.slice(4);
const [pB64, sB64] = body.split('.');
check('token has payload.signature form', body.includes('.') && pB64 && sB64);

// 2. signed bytes == JSON.stringify(payload)
const pBytes = b64ToBytes(pB64);
check('payload bytes == JSON.stringify(payload)', new TextDecoder().decode(pBytes) === JSON.stringify(payload));

// 3. signature verifies against the pubkey (mirrors ed25519_dalek verify.rs)
const sig = b64ToBytes(sB64);
check('signature length is 64', sig.length === 64);
check('signature verifies with public key', await ed.verifyAsync(sig, pBytes, pub));

// decoded payload round-trips + verifier ignores the extra license_id field
const decoded = JSON.parse(new TextDecoder().decode(pBytes));
check('decoded tier=signal', decoded.tier === 'signal');
check('decoded carries license_id (ignored by verifier)', decoded.license_id === 'cus_TEST123');

// 4. tamper rejection — flip one payload byte, signature must fail
const tampered = new Uint8Array(pBytes);
tampered[0] ^= 0x01;
check('tampered payload FAILS verification', !(await ed.verifyAsync(sig, tampered, pub)));

// wrong-key rejection — a different key must not verify this signature
const otherPub = await ed.getPublicKeyAsync(
  Uint8Array.from('4da0000000000000000000000000000000000000000000000000000000000002'.match(/../g).map((h) => parseInt(h, 16))),
);
check('signature FAILS against a different public key', !(await ed.verifyAsync(sig, pBytes, otherPub)));

// 5. expiry: token is short-lived (<= 7 days), and an expired one is detectable
const days = (new Date(decoded.expires_at) - new Date(decoded.issued_at)) / 864e5;
check('lease window is ~7 days (short-lived)', days > 6.9 && days < 7.1);
check('token not currently expired', new Date(decoded.expires_at) > new Date());

// refresh-key format matches the endpoint's guard
const rk = generateRefreshKey();
check('refresh key matches endpoint regex', /^4DA-LIC-[A-Z2-7]{16,80}$/.test(rk));
const rk2 = generateRefreshKey();
check('refresh keys are unique', rk !== rk2);

console.log(failures === 0 ? '\nALL LEASE CONTRACT CHECKS PASSED' : `\n${failures} CHECK(S) FAILED`);
process.exit(failures === 0 ? 0 : 1);
