#!/usr/bin/env node
// =============================================================================
// STREETS E2E Test — Full Stripe → License → Rust Verification Chain
//
// Tests the entire billing pipeline:
//   1. Checkout session creation (all 3 tiers)
//   2. Simulated webhook: create customer + payment → license generation
//   3. Activate endpoint: retrieve license by email
//   4. License format + crypto verification (Node.js side)
//   5. Cross-platform Rust verification (via cargo test)
//
// Run: node site/test-e2e-stripe.mjs
// Requires: site/.env.test (pulled from Vercel)
// =============================================================================

import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import crypto from 'crypto';
import { execSync } from 'child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ---------------------------------------------------------------------------
// Load env vars from .env.test
// ---------------------------------------------------------------------------
function loadEnv(path) {
  const content = readFileSync(path, 'utf8');
  for (const line of content.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const eq = trimmed.indexOf('=');
    if (eq === -1) continue;
    const key = trimmed.slice(0, eq);
    let val = trimmed.slice(eq + 1);
    // Strip surrounding quotes
    if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
      val = val.slice(1, -1);
    }
    process.env[key] = val;
  }
}

loadEnv(resolve(__dirname, '.env.test'));

// ---------------------------------------------------------------------------
// Validate required env vars
// ---------------------------------------------------------------------------
const required = [
  'STRIPE_SECRET_KEY',
  'LICENSE_PRIVATE_KEY_HEX',
  'STREETS_PRICE_COMMUNITY',
  'STREETS_PRICE_ANNUAL',
  'STREETS_PRICE_COHORT',
  'SITE_URL',
];

const missing = required.filter(k => !process.env[k]);
if (missing.length > 0) {
  console.error(`Missing env vars: ${missing.join(', ')}`);
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Init Stripe
// ---------------------------------------------------------------------------
const Stripe = (await import('stripe')).default;
const stripe = new Stripe(process.env.STRIPE_SECRET_KEY);

// Verify we're in test mode
if (!process.env.STRIPE_SECRET_KEY.startsWith('sk_test_')) {
  console.error('FATAL: Not using Stripe test key! Aborting.');
  process.exit(1);
}

const SITE_URL = process.env.SITE_URL;
const TEST_EMAIL = `e2e-test-${Date.now()}@4da-test.dev`;

let passed = 0;
let failed = 0;

function assert(condition, msg) {
  if (condition) {
    console.log(`  PASS  ${msg}`);
    passed++;
  } else {
    console.error(`  FAIL  ${msg}`);
    failed++;
  }
}

// ---------------------------------------------------------------------------
// Ed25519 license generation (mirrors activate.js)
// ---------------------------------------------------------------------------
function generateLicenseKey(payload) {
  const privateKeyHex = process.env.LICENSE_PRIVATE_KEY_HEX;
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

// Verify signature with public key (mirrors Rust verifier)
function verifyLicenseKey(licenseKey) {
  const PUBLIC_KEY_HEX = '084dc1b1b9549bf0ddff11db9186cb623ceb9d72831fbf2e6f01db160388f9d6';

  if (!licenseKey.startsWith('4DA-')) throw new Error('Missing 4DA- prefix');
  const body = licenseKey.slice(4);
  const dotIdx = body.lastIndexOf('.');
  if (dotIdx === -1) throw new Error('Missing signature separator');

  const payloadB64 = body.slice(0, dotIdx);
  const sigB64 = body.slice(dotIdx + 1);

  const payloadBytes = Buffer.from(payloadB64, 'base64');
  const sigBytes = Buffer.from(sigB64, 'base64');

  const publicKeyBuffer = Buffer.from(PUBLIC_KEY_HEX, 'hex');
  const keyObject = crypto.createPublicKey({
    key: Buffer.concat([
      Buffer.from('302a300506032b6570032100', 'hex'),
      publicKeyBuffer,
    ]),
    format: 'der',
    type: 'spki',
  });

  const valid = crypto.verify(null, payloadBytes, keyObject, sigBytes);
  if (!valid) throw new Error('Signature verification failed');

  return JSON.parse(payloadBytes.toString('utf8'));
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------
async function cleanupTestCustomer(email) {
  const customers = await stripe.customers.list({ email: email.toLowerCase(), limit: 10 });
  for (const c of customers.data) {
    // Cancel any active subscriptions first
    const subs = await stripe.subscriptions.list({ customer: c.id, status: 'active', limit: 10 });
    for (const sub of subs.data) {
      await stripe.subscriptions.cancel(sub.id);
    }
    await stripe.customers.del(c.id);
  }
}

// =============================================================================
// TEST 1: Checkout Session Creation (all 3 tiers)
// =============================================================================
console.log('\n--- Test 1: Checkout Session Creation ---');

for (const tier of ['community', 'annual', 'cohort']) {
  try {
    const resp = await fetch(`${SITE_URL}/api/streets/checkout`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tier }),
    });
    const data = await resp.json();
    assert(resp.ok && data.url && data.url.includes('checkout.stripe.com'), `${tier} checkout returns Stripe URL`);
  } catch (err) {
    assert(false, `${tier} checkout: ${err.message}`);
  }
}

// Invalid tier
try {
  const resp = await fetch(`${SITE_URL}/api/streets/checkout`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ tier: 'invalid' }),
  });
  assert(resp.status === 400, 'Invalid tier returns 400');
} catch (err) {
  assert(false, `Invalid tier test: ${err.message}`);
}

// =============================================================================
// TEST 2: Full Purchase Simulation (Customer → Payment → License)
// =============================================================================
console.log('\n--- Test 2: Simulated Purchase → License Generation ---');

// Clean up any leftover test customers
await cleanupTestCustomer(TEST_EMAIL);

// Create test customer
const customer = await stripe.customers.create({
  email: TEST_EMAIL,
  name: 'E2E Test User',
  metadata: { test: 'true' },
});
assert(!!customer.id, `Test customer created: ${customer.id}`);

// Attach test payment method (4242 card)
const pm = await stripe.paymentMethods.create({
  type: 'card',
  card: { token: 'tok_visa' },
});
await stripe.paymentMethods.attach(pm.id, { customer: customer.id });
await stripe.customers.update(customer.id, {
  invoice_settings: { default_payment_method: pm.id },
});
assert(true, 'Test card attached (4242...4242)');

// Create subscription (Community tier)
const subscription = await stripe.subscriptions.create({
  customer: customer.id,
  items: [{ price: process.env.STREETS_PRICE_COMMUNITY }],
  metadata: { streets_tier: 'signal', test: 'true' },
  default_payment_method: pm.id,
});
assert(subscription.status === 'active', `Subscription active: ${subscription.id}`);

// Now simulate what the webhook handler does: generate license and store in metadata
const now = new Date();
const expiresAt = new Date(now);
expiresAt.setFullYear(expiresAt.getFullYear() + 1);

const licensePayload = {
  tier: 'signal',
  email: TEST_EMAIL,
  expires_at: expiresAt.toISOString(),
  issued_at: now.toISOString(),
  features: ['signal'],
};

const licenseKey = generateLicenseKey(licensePayload);
assert(licenseKey.startsWith('4DA-'), 'License key has 4DA- prefix');
assert(licenseKey.length < 500, `License key within Stripe metadata limit (${licenseKey.length} chars)`);
assert(licenseKey.includes('.'), 'License key has payload.signature format');

// Store in customer metadata (as webhook does)
await stripe.customers.update(customer.id, {
  metadata: {
    streets_license: licenseKey,
    streets_tier: 'signal',
    streets_issued_at: now.toISOString(),
    streets_expires_at: expiresAt.toISOString(),
    streets_status: 'active',
  },
});
assert(true, 'License stored in Stripe customer metadata');

// =============================================================================
// TEST 3: License Retrieval via Activate Endpoint
// =============================================================================
console.log('\n--- Test 3: Activate Endpoint — License Retrieval ---');

try {
  const resp = await fetch(`${SITE_URL}/api/streets/activate?email=${encodeURIComponent(TEST_EMAIL)}`);
  const data = await resp.json();
  assert(resp.ok, `GET /api/streets/activate returns 200`);
  assert(data.license_key === licenseKey, 'Retrieved license matches generated key');
  assert(data.tier === 'signal', 'Tier is signal');
  assert(data.status === 'active' || !data.status, `Status is active (got: ${data.status || 'undefined'})`);
  assert(!!data.expires_at, 'Has expiry date');
} catch (err) {
  assert(false, `Activate endpoint: ${err.message}`);
}

// Test with non-existent email
try {
  const resp = await fetch(`${SITE_URL}/api/streets/activate?email=nobody-${Date.now()}@test.dev`);
  assert(resp.status === 404, 'Non-existent email returns 404');
} catch (err) {
  assert(false, `404 test: ${err.message}`);
}

// Test with no params
try {
  const resp = await fetch(`${SITE_URL}/api/streets/activate`);
  assert(resp.status === 400, 'No params returns 400');
} catch (err) {
  assert(false, `400 test: ${err.message}`);
}

// =============================================================================
// TEST 4: License Crypto Verification (Node.js)
// =============================================================================
console.log('\n--- Test 4: License Crypto Verification (Node.js) ---');

try {
  const decoded = verifyLicenseKey(licenseKey);
  assert(decoded.tier === 'signal', 'Decoded tier matches');
  assert(decoded.email === TEST_EMAIL, 'Decoded email matches');
  assert(decoded.features.includes('signal'), 'Has signal feature');
  assert(!!decoded.expires_at, 'Has expires_at');
  assert(!!decoded.issued_at, 'Has issued_at');
} catch (err) {
  assert(false, `Crypto verification: ${err.message}`);
}

// Test tampered key is rejected
try {
  const tampered = licenseKey.slice(0, -5) + 'ZZZZZ';
  verifyLicenseKey(tampered);
  assert(false, 'Tampered key should fail verification');
} catch {
  assert(true, 'Tampered key correctly rejected');
}

// Test wrong prefix rejected
try {
  verifyLicenseKey('FAKE-' + licenseKey.slice(4));
  assert(false, 'Wrong prefix should fail');
} catch {
  assert(true, 'Wrong prefix correctly rejected');
}

// =============================================================================
// TEST 5: Cross-Platform Rust Verification
// =============================================================================
console.log('\n--- Test 5: Rust Cross-Platform Verification ---');

try {
  // Write the license key to a temp file for Rust to read
  const { writeFileSync, unlinkSync } = await import('fs');
  const tmpPath = resolve(__dirname, '..', 'src-tauri', 'test-license-key.tmp');
  writeFileSync(tmpPath, licenseKey);

  // Run the existing Rust test that verifies JS-generated licenses
  const rustOutput = execSync(
    'cargo test --lib settings::license::tests::verify_js_generated_license -- --nocapture 2>&1',
    { cwd: resolve(__dirname, '..', 'src-tauri'), encoding: 'utf8', timeout: 120000 }
  );
  assert(rustOutput.includes('ok'), 'Rust verify_js_generated_license test passes');

  // Run all license tests
  const allTests = execSync(
    'cargo test --lib settings::license::tests -- --nocapture 2>&1',
    { cwd: resolve(__dirname, '..', 'src-tauri'), encoding: 'utf8', timeout: 120000 }
  );
  const testMatch = allTests.match(/test result: ok\. (\d+) passed/);
  assert(testMatch && parseInt(testMatch[1]) >= 5, `All ${testMatch?.[1] || '?'} Rust license tests pass`);

  // Clean up
  try { unlinkSync(tmpPath); } catch {}
} catch (err) {
  assert(false, `Rust verification: ${err.message}`);
}

// =============================================================================
// TEST 6: Cohort License (One-time payment tier)
// =============================================================================
console.log('\n--- Test 6: Cohort License Generation ---');

// Legacy cohort keys should still verify (backwards compat)
const cohortPayload = {
  tier: 'signal',
  email: TEST_EMAIL,
  expires_at: expiresAt.toISOString(),
  issued_at: now.toISOString(),
  features: ['signal'],
};

const cohortKey = generateLicenseKey(cohortPayload);
assert(cohortKey.startsWith('4DA-'), 'Signal key has prefix');
assert(cohortKey.length < 500, `Signal key within limit (${cohortKey.length} chars)`);

try {
  const decoded = verifyLicenseKey(cohortKey);
  assert(decoded.tier === 'signal', 'Signal tier decoded');
  assert(decoded.features.includes('signal'), 'Has signal feature');
} catch (err) {
  assert(false, `Signal verification: ${err.message}`);
}

// =============================================================================
// TEST 7: Deep-Link URL Format
// =============================================================================
console.log('\n--- Test 7: Deep-Link URL Format ---');

const deepLink = `4da://activate?key=${encodeURIComponent(licenseKey)}`;
assert(deepLink.startsWith('4da://activate?key=4DA-'), 'Deep-link has correct protocol and prefix');

// Node.js URL parser doesn't support custom protocols directly.
// Use a http:// substitution to validate the query param round-trips correctly.
const httpEquiv = deepLink.replace('4da://', 'http://localhost/');
const parsedUrl = new URL(httpEquiv);
assert(parsedUrl.searchParams.get('key') === licenseKey, 'Key round-trips through URL encoding');

// Verify the actual deep-link structure matches what activate.html generates
assert(deepLink.includes('4da://activate?key='), 'Deep-link matches 4da://activate?key= format');
const decodedKey = decodeURIComponent(deepLink.split('key=')[1]);
assert(decodedKey === licenseKey, 'URL-decoded key matches original');

// =============================================================================
// TEST 8: Subscription Cancellation Flow
// =============================================================================
console.log('\n--- Test 8: Subscription Cancellation ---');

// Cancel the subscription
const cancelled = await stripe.subscriptions.cancel(subscription.id);
assert(cancelled.status === 'canceled', 'Subscription cancelled');

// Simulate what the webhook does: mark status in metadata
await stripe.customers.update(customer.id, {
  metadata: {
    streets_status: 'cancelled',
    streets_cancelled_at: new Date().toISOString(),
  },
});

// License should still be retrievable (valid until expires_at)
try {
  const resp = await fetch(`${SITE_URL}/api/streets/activate?email=${encodeURIComponent(TEST_EMAIL)}`);
  const data = await resp.json();
  assert(resp.ok, 'Cancelled license still retrievable');
  assert(data.license_key === licenseKey, 'Key still valid after cancellation');
  assert(data.status === 'cancelled', 'Status shows cancelled');
} catch (err) {
  assert(false, `Cancellation retrieval: ${err.message}`);
}

// =============================================================================
// TEST 9: Expired License Handling
// =============================================================================
console.log('\n--- Test 9: Expired License ---');

// Set the expiry to the past
const pastDate = new Date();
pastDate.setFullYear(pastDate.getFullYear() - 1);

await stripe.customers.update(customer.id, {
  metadata: {
    streets_expires_at: pastDate.toISOString(),
  },
});

try {
  const resp = await fetch(`${SITE_URL}/api/streets/activate?email=${encodeURIComponent(TEST_EMAIL)}`);
  assert(resp.status === 410, 'Expired license returns HTTP 410 Gone');
  const data = await resp.json();
  assert(data.error && data.error.includes('expired'), 'Error message mentions expiry');
} catch (err) {
  assert(false, `Expired license test: ${err.message}`);
}

// =============================================================================
// CLEANUP
// =============================================================================
console.log('\n--- Cleanup ---');
await cleanupTestCustomer(TEST_EMAIL);
console.log('  Test customer deleted');

// =============================================================================
// RESULTS
// =============================================================================
console.log('\n' + '='.repeat(60));
console.log(`  RESULTS: ${passed} passed, ${failed} failed, ${passed + failed} total`);
console.log('='.repeat(60));

if (failed > 0) {
  console.error('\nE2E TEST SUITE FAILED');
  process.exit(1);
} else {
  console.log('\nALL E2E TESTS PASSED');
  process.exit(0);
}
