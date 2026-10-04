// Silent renewal decisions. Dependency-free; runs in `pnpm run test:scripts`.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  PAST_DUE_LEASE_DAYS,
  RENEWAL_GRACE_DAYS,
  classifyPresentedKey,
  extendsAccess,
  parseLicenseKey,
  pickEntitlingSubscription,
  renewedExpiry,
  renewedExpiryFor,
  subscriptionPeriodEnd,
} from './renewal.js';

const DAY = 24 * 60 * 60 * 1000;
const b64 = (s) => Buffer.from(s).toString('base64');
const SIG = Buffer.alloc(64, 7).toString('base64');
const keyFor = (payload) => `4DA-${b64(JSON.stringify(payload))}.${SIG}`;
const NOW = Date.parse('2026-10-04T00:00:00Z');

test('parses a well-formed 4DA key into payload + signature bytes', () => {
  const payload = { tier: 'signal', email: 'a@b.co', expires_at: '2026-11-08T00:00:00Z', issued_at: 'x', features: ['signal'] };
  const parsed = parseLicenseKey(keyFor(payload));
  assert.ok(parsed);
  assert.equal(parsed.payload.email, 'a@b.co');
  assert.equal(parsed.sigBytes.length, 64);
});

test('tolerates line breaks pasted from an email', () => {
  const k = keyFor({ email: 'a@b.co', expires_at: '2026-11-08T00:00:00Z' });
  assert.ok(parseLicenseKey(`${k.slice(0, 40)}\n${k.slice(40)}`));
});

test('rejects refresh credentials, junk, wrong-size signatures and payloads without email/expiry', () => {
  assert.equal(parseLicenseKey('4DA-LIC-ABCDEFGHIJKLMNOPQRSTUVWX'), null);
  assert.equal(parseLicenseKey('hello'), null);
  assert.equal(parseLicenseKey(null), null);
  assert.equal(parseLicenseKey('4DA-' + 'A'.repeat(2000) + '.' + SIG), null);
  assert.equal(parseLicenseKey(`4DA-${b64('{"email":"a@b.co","expires_at":"2026-01-01"}')}.${b64('short')}`), null);
  assert.equal(parseLicenseKey(`4DA-${b64('{"tier":"signal"}')}.${SIG}`), null);
  assert.equal(parseLicenseKey(`4DA-${b64('not json')}.${SIG}`), null);
});

test('lifetime keys never renew', () => {
  assert.equal(classifyPresentedKey({ expires_at: '2099-10-04T13:32:22.574Z' }), 'lifetime');
});

test('a subscription key of any age is a renewal credential — the live subscription is the only gate', () => {
  assert.equal(classifyPresentedKey({ expires_at: new Date(NOW + 3 * DAY).toISOString() }), 'eligible');
  assert.equal(classifyPresentedKey({ expires_at: new Date(NOW - 400 * DAY).toISOString() }), 'eligible', 'the key from the purchase email, a year on');
  assert.equal(classifyPresentedKey({ expires_at: 'garbage' }), 'invalid');
});

test('period end is read from items on API basil+ and from the subscription on older versions', () => {
  assert.equal(subscriptionPeriodEnd({ items: { data: [{ current_period_end: 100 }, { current_period_end: 300 }] } }), 300);
  assert.equal(subscriptionPeriodEnd({ current_period_end: 200, items: { data: [{}] } }), 200);
  assert.equal(subscriptionPeriodEnd({}), null);
});

test('only active / trialing / past_due subscriptions entitle; the latest paid period wins', () => {
  const subs = [
    { status: 'canceled', items: { data: [{ current_period_end: 999 }] } },
    { status: 'active', items: { data: [{ current_period_end: 500 }] } },
    { status: 'past_due', items: { data: [{ current_period_end: 700 }] } },
    { status: 'incomplete_expired', items: { data: [{ current_period_end: 900 }] } },
  ];
  assert.equal(subscriptionPeriodEnd(pickEntitlingSubscription(subs)), 700);
  assert.equal(pickEntitlingSubscription([{ status: 'canceled', current_period_end: 1 }]), null);
  assert.equal(pickEntitlingSubscription([]), null);
});

test('a cancel-at-period-end subscription still renews up to the end of what was paid', () => {
  const sub = { status: 'active', cancel_at_period_end: true, items: { data: [{ current_period_end: NOW / 1000 + 10 * 86400 }] } };
  const end = subscriptionPeriodEnd(pickEntitlingSubscription([sub]));
  assert.equal(renewedExpiry(end).getTime(), NOW + (10 + RENEWAL_GRACE_DAYS) * DAY);
});

test('a renewal is only handed out when it actually extends access', () => {
  const presented = '2026-11-08T00:00:00.000Z';
  assert.equal(extendsAccess(presented, new Date('2026-12-15T00:00:00Z')), true);
  assert.equal(extendsAccess(presented, new Date('2026-11-08T00:30:00Z')), false, 'within 1h is the same key');
  assert.equal(extendsAccess(presented, new Date('2026-11-01T00:00:00Z')), false);
  assert.equal(extendsAccess(presented, 'not a date'), false);
});

test('a paying subscription renews to period end + grace', () => {
  const end = NOW / 1000 + 30 * 86400;
  const sub = { status: 'active', items: { data: [{ current_period_end: end }] } };
  assert.equal(renewedExpiryFor(sub, NOW).getTime(), NOW + (30 + RENEWAL_GRACE_DAYS) * DAY);
});

test('a failing card (past_due) gets a short lease, never the period Stripe is still trying to charge for', () => {
  const end = NOW / 1000 + 30 * 86400;
  const sub = { status: 'past_due', items: { data: [{ current_period_end: end }] } };
  assert.equal(renewedExpiryFor(sub, NOW).getTime(), NOW + PAST_DUE_LEASE_DAYS * DAY);
  const nearEnd = { status: 'past_due', items: { data: [{ current_period_end: NOW / 1000 - 6 * 86400 }] } };
  assert.equal(renewedExpiryFor(nearEnd, NOW).getTime(), NOW + (RENEWAL_GRACE_DAYS - 6) * DAY, 'never past the paid end + grace');
});
