#!/usr/bin/env node
// Generate a self-signed 4DA license key for manual issuance.
//
// Usage:
//   LICENSE_PRIVATE_KEY_HEX=<hex> node scripts/generate-license.mjs --email user@example.com
//   LICENSE_PRIVATE_KEY_HEX=<hex> node scripts/generate-license.mjs --email user@example.com --tier team --days 365
//
// Environment:
//   LICENSE_PRIVATE_KEY_HEX  Ed25519 private key (64 hex chars / 32 bytes)
//
// The generated key is verified locally before output to ensure Rust compatibility.

import crypto from 'crypto';

// ---------------------------------------------------------------------------
// Ed25519 license generation (same logic as site/api/streets/activate.js)
// ---------------------------------------------------------------------------

function generateLicenseKey(payload, privateKeyHex) {
  const payloadJson = JSON.stringify(payload);
  const payloadBytes = Buffer.from(payloadJson, 'utf8');
  const payloadB64 = payloadBytes.toString('base64');

  const privateKeyBuffer = Buffer.from(privateKeyHex, 'hex');
  const keyObject = crypto.createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      privateKeyBuffer,
    ]),
    format: 'der',
    type: 'pkcs8',
  });

  const signature = crypto.sign(null, payloadBytes, keyObject);
  const sigB64 = signature.toString('base64');

  return `4DA-${payloadB64}.${sigB64}`;
}

// Derive public key and verify the signature locally
function verifyLicenseKey(licenseKey, privateKeyHex) {
  const privateKeyBuffer = Buffer.from(privateKeyHex, 'hex');
  const keyObject = crypto.createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      privateKeyBuffer,
    ]),
    format: 'der',
    type: 'pkcs8',
  });
  const publicKey = crypto.createPublicKey(keyObject);

  const body = licenseKey.replace('4DA-', '');
  const [payloadB64, sigB64] = body.split('.');
  const payloadBytes = Buffer.from(payloadB64, 'base64');
  const sigBytes = Buffer.from(sigB64, 'base64');

  return crypto.verify(null, payloadBytes, publicKey, sigBytes);
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const args = process.argv.slice(2);

function getArg(name, fallback) {
  const idx = args.indexOf(`--${name}`);
  if (idx === -1 || idx + 1 >= args.length) return fallback;
  return args[idx + 1];
}

const privateKeyHex = process.env.LICENSE_PRIVATE_KEY_HEX;
if (!privateKeyHex) {
  console.error('Error: LICENSE_PRIVATE_KEY_HEX environment variable is required.');
  console.error('Usage: LICENSE_PRIVATE_KEY_HEX=<hex> node scripts/generate-license.mjs --email user@example.com');
  process.exit(1);
}

if (privateKeyHex.length !== 64) {
  console.error(`Error: LICENSE_PRIVATE_KEY_HEX must be 64 hex characters (32 bytes). Got ${privateKeyHex.length}.`);
  process.exit(1);
}

const email = getArg('email', null);
if (!email) {
  console.error('Error: --email is required.');
  console.error('Usage: LICENSE_PRIVATE_KEY_HEX=<hex> node scripts/generate-license.mjs --email user@example.com');
  process.exit(1);
}

const tier = getArg('tier', 'signal');
const days = parseInt(getArg('days', '365'), 10);
const features = [tier];

const now = new Date();
const expiresAt = new Date(now);
expiresAt.setDate(expiresAt.getDate() + days);

const payload = {
  tier,
  email,
  expires_at: expiresAt.toISOString(),
  issued_at: now.toISOString(),
  features,
};

const licenseKey = generateLicenseKey(payload, privateKeyHex);

// Verify before output
const valid = verifyLicenseKey(licenseKey, privateKeyHex);
if (!valid) {
  console.error('Error: Generated key failed local verification. Check your private key.');
  process.exit(1);
}

console.log('');
console.log('4DA License Key Generated');
console.log('='.repeat(60));
console.log(`Tier:       ${tier}`);
console.log(`Email:      ${email}`);
console.log(`Issued:     ${now.toISOString()}`);
console.log(`Expires:    ${expiresAt.toISOString()} (${days} days)`);
console.log(`Features:   ${features.join(', ')}`);
console.log(`Verified:   ${valid ? 'YES' : 'FAILED'}`);
console.log(`Length:     ${licenseKey.length} chars`);
console.log('');
console.log('Key:');
console.log(licenseKey);
console.log('');
