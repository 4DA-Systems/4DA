// The licence-signing health check: the site's copy of the app public key must be
// the one the app actually compiles in, and the report must only go green on an
// exact match. Dependency-free; runs in `pnpm run test:scripts`.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { APP_LICENSE_PUBLIC_KEY_HEX, signingKeyReport } from './license-health.js';

test('the site pins the SAME public key the desktop app verifies against', () => {
  const verifyRs = readFileSync(
    new URL('../../src-tauri/src/settings/license/verify.rs', import.meta.url),
    'utf8',
  );
  const m = verifyRs.match(/LICENSE_PUBLIC_KEY_HEX:\s*&str\s*=\s*"([0-9a-f]{64})"/);
  assert.ok(m, 'LICENSE_PUBLIC_KEY_HEX not found in verify.rs — update this test with the new location');
  assert.equal(APP_LICENSE_PUBLIC_KEY_HEX, m[1]);
});

test('an exact match is healthy', () => {
  const r = signingKeyReport({ configured: true, derivedPublicKeyHex: APP_LICENSE_PUBLIC_KEY_HEX });
  assert.equal(r.ok, true);
  assert.equal(r.signing_key.matches_app, true);
});

test('hex case does not matter', () => {
  const r = signingKeyReport({ configured: true, derivedPublicKeyHex: APP_LICENSE_PUBLIC_KEY_HEX.toUpperCase() });
  assert.equal(r.ok, true);
});

test('a different key is unhealthy and shows what it derived', () => {
  const other = 'ab'.repeat(32);
  const r = signingKeyReport({ configured: true, derivedPublicKeyHex: other });
  assert.equal(r.ok, false);
  assert.equal(r.signing_key.matches_app, false);
  assert.equal(r.signing_key.derived_public_key, other);
});

test('a missing or unparseable secret is unhealthy, never green', () => {
  assert.equal(signingKeyReport({ configured: false }).ok, false);
  const bad = signingKeyReport({ configured: true, error: 'seed could not be parsed' });
  assert.equal(bad.ok, false);
  assert.equal(bad.signing_key.error, 'seed could not be parsed');
});

test('the report never carries anything but public material', () => {
  const r = signingKeyReport({ configured: true, derivedPublicKeyHex: APP_LICENSE_PUBLIC_KEY_HEX });
  assert.deepEqual(Object.keys(r.signing_key).sort(), [
    'configured',
    'derived_public_key',
    'expected_public_key',
    'matches_app',
  ]);
});
