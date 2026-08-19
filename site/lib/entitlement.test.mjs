// Regression tests for the money path: refunds, chargebacks and the customer
// resolution that feeds licence issuance.
//
// WHY THIS FILE EXISTS AS A DEPENDENCY-FREE `node --test` SUITE:
// site/ has its own pnpm-lock.yaml and is never `pnpm install`ed by any CI job,
// so any test that imported `stripe` or `@noble/ed25519` could not run in CI at
// all. lib/entitlement.js therefore imports nothing, and this suite runs from
// `pnpm run test:scripts` — which lives in validate.yml's `repo-guards` job,
// the one job with no path filter and no `needs:`. That is the first PR-time
// test gate the Cloudflare payment path has ever had.
//
// What each test would catch if the fix were reverted:
//   - "refunded is a terminal status"    -> the reader/writer mismatch: nothing
//                                           in the repo ever wrote 'refunded',
//                                           so refresh.js's check for it was
//                                           dead code that looked alive.
//   - "duplicate delivery is a no-op"    -> the missing event-id dedup store.
//   - "severity never downgrades"        -> out-of-order Stripe deliveries.
//   - "resolveCustomerId guards first"   -> the TypeError-instead-of-error bug.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  TERMINAL_STATUSES,
  hasOtherStandingCharge,
  meta,
  metaKey,
  isLifetimeEntitled,
  isRevoked,
  isTerminal,
  resolveCustomerId,
  severityOf,
  stripeIdOf,
  terminalStatusPatch,
} from './entitlement.js';

// ---------------------------------------------------------------------------
// The reader/writer contract
// ---------------------------------------------------------------------------

test('refunded and chargeback are terminal, so a lifetime holder loses entitlement', () => {
  assert.ok(TERMINAL_STATUSES.includes('refunded'), "'refunded' must be a terminal status");
  assert.ok(TERMINAL_STATUSES.includes('chargeback'), "'chargeback' must be a terminal status");

  const base = { streets_billing_period: 'lifetime' };
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'active' }), true);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'refunded' }), false);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'chargeback' }), false);
  assert.equal(isLifetimeEntitled({ ...base, streets_status: 'cancelled' }), false);
});

test('every terminal status is actually producible by terminalStatusPatch', () => {
  // The defect this guards: refresh.js gated on a status string that no code
  // path could ever write. Any status the reader rejects must be writable.
  for (const status of TERMINAL_STATUSES) {
    const patch = terminalStatusPatch({ streets_status: 'active' }, status, '2026-01-01T00:00:00.000Z');
    assert.equal(patch.signal_status, status, `${status} must be writable`);
  }
});

test('a non-lifetime customer is not lifetime-entitled whatever the status', () => {
  assert.equal(isLifetimeEntitled({ streets_billing_period: 'monthly', streets_status: 'active' }), false);
  assert.equal(isLifetimeEntitled({}), false);
  assert.equal(isLifetimeEntitled(null), false);
  assert.equal(isLifetimeEntitled(undefined), false);
});

// ---------------------------------------------------------------------------
// Idempotency by construction — the stand-in for the absent KV dedup store
// ---------------------------------------------------------------------------

test('duplicate delivery of the same refund writes identical bytes', () => {
  const first = terminalStatusPatch(
    { streets_billing_period: 'lifetime', streets_status: 'active' },
    'refunded',
    '2026-03-01T10:00:00.000Z',
  );
  assert.deepEqual(first, { signal_refunded_at: '2026-03-01T10:00:00.000Z', signal_status: 'refunded' });

  // Stripe re-delivers. The customer now already carries the first patch.
  const after = { streets_billing_period: 'lifetime', ...first };
  const second = terminalStatusPatch(after, 'refunded', '2026-03-01T10:07:31.000Z');

  assert.equal(second.signal_refunded_at, '2026-03-01T10:00:00.000Z', 'first-seen timestamp is preserved');
  assert.equal(second.signal_status, undefined, 'no status rewrite on a repeat');
  assert.equal(isLifetimeEntitled({ ...after, ...second }), false);
});

test('a trailing cancellation does not downgrade a chargeback', () => {
  // Stripe emits customer.subscription.deleted AFTER charge.dispute.created for
  // the same customer. Last-write-wins would report a plain cancellation.
  const disputed = terminalStatusPatch({ streets_status: 'active' }, 'chargeback', '2026-03-01T10:00:00.000Z');
  const state = { ...disputed };
  const trailing = terminalStatusPatch(state, 'cancelled', '2026-03-01T10:05:00.000Z');

  assert.equal(trailing.signal_status, undefined, 'chargeback must not be downgraded to cancelled');
  assert.equal(trailing.signal_cancelled_at, '2026-03-01T10:05:00.000Z', 'the cancellation is still recorded');
  assert.equal({ ...state, ...trailing }.signal_status, 'chargeback');
});

test('a refund arriving after a cancellation escalates', () => {
  const cancelled = terminalStatusPatch({ streets_status: 'active' }, 'cancelled', '2026-03-01T10:00:00.000Z');
  const refunded = terminalStatusPatch({ ...cancelled }, 'refunded', '2026-03-02T10:00:00.000Z');
  assert.equal(refunded.signal_status, 'refunded');
  assert.equal(severityOf('refunded') > severityOf('cancelled'), true);
});

test('a refund in a NEW episode gets a fresh timestamp', () => {
  // Refunded, then re-purchased (generateAndStoreLicense writes 'active'),
  // then refunded again. The second refund must not report the first date.
  const state = {
    streets_status: 'active',
    streets_refunded_at: '2026-01-01T00:00:00.000Z',
    streets_billing_period: 'lifetime',
  };
  const patch = terminalStatusPatch(state, 'refunded', '2026-06-01T00:00:00.000Z');
  assert.equal(patch.signal_refunded_at, '2026-06-01T00:00:00.000Z');
  assert.equal(patch.signal_status, 'refunded');
});

test('terminalStatusPatch refuses a status that is not terminal', () => {
  assert.throws(() => terminalStatusPatch({}, 'active', '2026-01-01T00:00:00.000Z'), /not a terminal status/);
  assert.throws(() => terminalStatusPatch({}, 'refunded_maybe', '2026-01-01T00:00:00.000Z'), /not a terminal status/);
});

// ---------------------------------------------------------------------------
// Stripe reference unwrapping
// ---------------------------------------------------------------------------

test('stripeIdOf handles id strings, expanded objects and absent refs', () => {
  assert.equal(stripeIdOf('cus_123'), 'cus_123');
  assert.equal(stripeIdOf({ id: 'ch_456', object: 'charge' }), 'ch_456');
  assert.equal(stripeIdOf(null), null);
  assert.equal(stripeIdOf(undefined), null);
  assert.equal(stripeIdOf({}), null);
});

// ---------------------------------------------------------------------------
// resolveCustomerId ordering
// ---------------------------------------------------------------------------

function fakeStripe({ existing = [], createdId = 'cus_created' } = {}) {
  const calls = { list: [], create: [] };
  return {
    calls,
    customers: {
      async list(args) {
        calls.list.push(args);
        return { data: existing };
      },
      async create(args) {
        calls.create.push(args);
        return { id: createdId };
      },
    },
  };
}

test('resolveCustomerId throws a NAMED error, not a TypeError, with no id and no email', async () => {
  // The bug: `email.toLowerCase()` ran before the caller's `if (!email)` guard,
  // so this path produced "Cannot read properties of null (reading
  // 'toLowerCase')" and the webhook answered a bare 500 Stripe kept retrying.
  const stripe = fakeStripe();
  await assert.rejects(() => resolveCustomerId(stripe, null, null), (err) => {
    assert.ok(!(err instanceof TypeError), `must not be a TypeError, got: ${err}`);
    assert.match(err.message, /no customer id and no email/);
    return true;
  });
  assert.equal(stripe.calls.list.length, 0, 'must not reach Stripe with a null email');
  assert.equal(stripe.calls.create.length, 0);
});

test('resolveCustomerId short-circuits on an existing customer id', async () => {
  const stripe = fakeStripe();
  assert.equal(await resolveCustomerId(stripe, 'cus_existing', null), 'cus_existing');
  assert.equal(stripe.calls.list.length, 0);
});

test('resolveCustomerId normalises the email for both lookup and creation', async () => {
  const found = fakeStripe({ existing: [{ id: 'cus_found' }] });
  assert.equal(await resolveCustomerId(found, null, 'Buyer@Example.COM'), 'cus_found');
  assert.deepEqual(found.calls.list[0], { email: 'buyer@example.com', limit: 1 });

  const fresh = fakeStripe({ existing: [] });
  assert.equal(await resolveCustomerId(fresh, null, 'Buyer@Example.COM'), 'cus_created');
  assert.deepEqual(fresh.calls.create[0], { email: 'buyer@example.com' });
});

// ---------------------------------------------------------------------------
// Purchase-scoped termination.
//
// Terminal status is written on the CUSTOMER; a refund happens to a CHARGE.
// Conflating the two revokes a customer's whole entitlement when support
// refunds a duplicate charge. These pin the distinction.
// ---------------------------------------------------------------------------

function chargeStripe(charges) {
  const calls = [];
  return {
    calls,
    charges: {
      async list(args) {
        calls.push(args);
        return { data: charges };
      },
    },
  };
}

const paid = (id) => ({ id, paid: true, status: 'succeeded', refunded: false, disputed: false });
const refunded = (id) => ({ id, paid: true, status: 'succeeded', refunded: true, disputed: false });

test('a duplicate charge refunded leaves the surviving payment standing', async () => {
  // The $299 lifetime purchase plus an accidental second charge; support
  // refunds the duplicate. The original must still entitle them.
  const stripe = chargeStripe([paid('ch_original'), refunded('ch_duplicate')]);
  assert.equal(await hasOtherStandingCharge(stripe, 'cus_1', 'ch_duplicate'), true);
  assert.deepEqual(stripe.calls[0], { customer: 'cus_1', limit: 100 });
});

test('the only charge being refunded leaves nothing standing', async () => {
  const stripe = chargeStripe([refunded('ch_only')]);
  assert.equal(await hasOtherStandingCharge(stripe, 'cus_1', 'ch_only'), false);
});

test('the charge under processing is never counted as standing', async () => {
  // Stripe may still report it as unrefunded when the event arrives.
  const stripe = chargeStripe([paid('ch_being_refunded')]);
  assert.equal(await hasOtherStandingCharge(stripe, 'cus_1', 'ch_being_refunded'), false);
});

test('failed, unpaid and disputed charges do not count as standing', async () => {
  const stripe = chargeStripe([
    { id: 'ch_failed', paid: false, status: 'failed', refunded: false, disputed: false },
    { id: 'ch_pending', paid: true, status: 'pending', refunded: false, disputed: false },
    { id: 'ch_disputed', paid: true, status: 'succeeded', refunded: false, disputed: true },
  ]);
  assert.equal(await hasOtherStandingCharge(stripe, 'cus_1', 'ch_x'), false);
});

test('a charge-lookup failure does not revoke', async () => {
  // Fail closed on entitlement: wrongly keeping access is a support ticket,
  // wrongly revoking a paying customer is a refund and a lost customer.
  const stripe = {
    charges: {
      async list() {
        throw new Error('stripe unreachable');
      },
    },
  };
  assert.equal(await hasOtherStandingCharge(stripe, 'cus_1', 'ch_x'), true);
});

// ---------------------------------------------------------------------------
// A renewal must not resurrect a terminated customer.
// ---------------------------------------------------------------------------

test('isTerminal recognises every terminal status and nothing else', () => {
  for (const s of TERMINAL_STATUSES) {
    assert.equal(isTerminal({ signal_status: s }), true, `${s} must be terminal`);
  }
  assert.equal(isTerminal({ signal_status: 'active' }), false);
  assert.equal(isTerminal({}), false);
  assert.equal(isTerminal(null), false);
});

// ---------------------------------------------------------------------------
// Metadata namespace migration: streets_* -> signal_*
//
// The rename exists because `streets_*` read as a retired feature and so made
// the LIVE licensing path look retired too — it caused a real wrong call about
// whether this endpoint was still in use. Writes moved to `signal_*`; reads must
// still accept `streets_*` so no existing customer record is orphaned.
//
// These pin the compatibility guarantee rather than trusting it. Delete them
// only when the legacy fallback itself is deleted.
// ---------------------------------------------------------------------------

test('a LEGACY-only customer record is still read correctly', () => {
  const legacyOnly = {
    streets_billing_period: 'lifetime',
    streets_status: 'active',
    streets_license: '4DA-legacy',
    streets_tier: 'signal',
  };
  assert.equal(meta(legacyOnly, 'billing_period'), 'lifetime');
  assert.equal(meta(legacyOnly, 'status'), 'active');
  assert.equal(meta(legacyOnly, 'license'), '4DA-legacy');
  assert.equal(isLifetimeEntitled(legacyOnly), true, 'a legacy lifetime holder keeps entitlement');
  assert.equal(isTerminal(legacyOnly), false);
});

test('a legacy record that went terminal is still recognised as terminal', () => {
  // The failure this prevents: reading only signal_* would report an old
  // refunded customer as entitled, silently re-granting revoked access.
  const legacyRefunded = { streets_billing_period: 'lifetime', streets_status: 'refunded' };
  assert.equal(isTerminal(legacyRefunded), true);
  assert.equal(isLifetimeEntitled(legacyRefunded), false);
});

test('the CURRENT prefix wins when a record carries both', () => {
  // Mid-migration a record can hold both: the webhook writes signal_status while
  // streets_status lingers from before. The new value is authoritative.
  const both = { streets_status: 'active', signal_status: 'refunded' };
  assert.equal(meta(both, 'status'), 'refunded');
  assert.equal(isTerminal(both), true);
});

test('writes use signal_*, and a terminal patch on a legacy record migrates it', () => {
  const legacy = { streets_billing_period: 'lifetime', streets_status: 'active' };
  const patch = terminalStatusPatch(legacy, 'refunded', '2026-05-01T00:00:00.000Z');
  assert.equal(patch.signal_status, 'refunded', 'the new key is written');
  assert.equal(patch.streets_status, undefined, 'the legacy key is never written again');
  assert.equal(patch.signal_refunded_at, '2026-05-01T00:00:00.000Z');
  // Stripe MERGES metadata, so the stale streets_status survives on the record.
  // meta() must therefore prefer the new key — proven above — or the merged
  // result would read as still-active.
  assert.equal(isTerminal({ ...legacy, ...patch }), true);
});

test('a legacy first-seen stamp is preserved rather than refreshed', () => {
  // Idempotency has to survive the rename: a re-delivered refund against a
  // record whose stamp is still streets_refunded_at must not restamp it.
  const legacyRefunded = {
    streets_status: 'refunded',
    streets_refunded_at: '2026-01-01T00:00:00.000Z',
  };
  const repeat = terminalStatusPatch(legacyRefunded, 'refunded', '2026-09-09T00:00:00.000Z');
  assert.equal(
    repeat.signal_refunded_at,
    '2026-01-01T00:00:00.000Z',
    'first-seen date carried across the rename',
  );
  assert.equal(repeat.signal_status, undefined, 'no status rewrite on a repeat');
});

test('metaKey builds the write key and nothing else', () => {
  assert.equal(metaKey('status'), 'signal_status');
  assert.equal(metaKey('license'), 'signal_license');
  assert.equal(meta({}, 'status'), undefined);
  assert.equal(meta(null, 'status'), undefined);
});

test('isRevoked means the money went back — stricter than isTerminal', () => {
  // Refund and chargeback revoke retrieval; a plain cancellation does NOT —
  // the cancelled subscriber paid for their current period, and blocking their
  // key recovery during that tail would take back something they bought.
  assert.equal(isRevoked({ signal_status: 'refunded' }), true);
  assert.equal(isRevoked({ signal_status: 'chargeback' }), true);
  assert.equal(isRevoked({ signal_status: 'cancelled' }), false);
  assert.equal(isRevoked({ signal_status: 'active' }), false);
  assert.equal(isRevoked({}), false);
  assert.equal(isRevoked(null), false);
  // Legacy-prefixed records read through the namespace fallback.
  assert.equal(isRevoked({ streets_status: 'refunded' }), true);
  assert.equal(isRevoked({ streets_status: 'cancelled' }), false);
});
