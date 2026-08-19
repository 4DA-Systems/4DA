// Shared entitlement-lifecycle helpers for the Stripe-backed licence system.
//
// Imported by BOTH the webhook that WRITES entitlement state
// (functions/api/streets/activate.js) and the lease endpoint that READS it
// (functions/api/license/refresh.js). The sharing is the whole point. Before
// this module existed, refresh.js gated lifetime access on
// `streets_status !== 'refunded'` while NOTHING anywhere in the repo ever wrote
// 'refunded' — a reader checking for a value no writer produced. The list of
// terminal statuses now has exactly one definition, and both sides use it, so
// the two cannot drift apart again.
//
// DELIBERATELY DEPENDENCY-FREE. It imports nothing, which is what lets the
// payment path carry a regression test at all: site/ has no installed
// dependency tree in CI, so a test that had to `import Stripe from 'stripe'`
// could never run there. See entitlement.test.mjs (wired into
// `pnpm run test:scripts`, which runs in the unfiltered `repo-guards` job).

/**
 * Statuses that END entitlement. Ordered weakest -> strongest; see SEVERITY.
 * Anything not in this list (notably 'active') is entitling.
 */
export const TERMINAL_STATUSES = ['cancelled', 'refunded', 'chargeback'];

// ---------------------------------------------------------------------------
// Metadata namespace
//
// These keys were written `streets_*` because the Signal tier was originally
// built on the STREETS scaffolding. STREETS was retired in June 2026 and the
// name has been actively misleading ever since — it reads as a dead feature, so
// the live licensing path looks dead too. (It cost a real wrong call: the
// endpoint was believed retired precisely because of this prefix.)
//
// Writes now use `signal_*`. Reads accept EITHER, preferring the current one,
// so any pre-existing customer record keeps working without a migration.
//
// The `streets_` fallback is deletable once no Stripe customer carries the old
// keys. Do not delete it on the assumption that none do — check first.
// ---------------------------------------------------------------------------

const META_PREFIX = 'signal_';
const LEGACY_META_PREFIX = 'streets_';

/** The metadata key to WRITE for a field, e.g. `metaKey('status')`. */
export function metaKey(field) {
  return META_PREFIX + field;
}

/** Read a namespaced entitlement field, preferring the current prefix. */
export function meta(metadata, field) {
  const md = metadata || {};
  const current = md[META_PREFIX + field];
  return current === undefined ? md[LEGACY_META_PREFIX + field] : current;
}

/**
 * Severity ranking. Webhook deliveries are NOT ordered — Stripe can deliver
 * `customer.subscription.deleted` after `charge.dispute.created` for the same
 * customer — so the write must converge on the most severe outcome regardless
 * of arrival order rather than letting the last message win.
 */
const SEVERITY = { active: 0, cancelled: 1, refunded: 2, chargeback: 3 };

/** Where each terminal status records WHEN it was first observed. */
const STAMP_KEY = {
  cancelled: metaKey('cancelled_at'),
  refunded: metaKey('refunded_at'),
  chargeback: metaKey('chargeback_at'),
};

/** Read a terminal-status stamp, tolerating the legacy prefix. */
function readStamp(metadata, status) {
  const field = { cancelled: 'cancelled_at', refunded: 'refunded_at', chargeback: 'chargeback_at' }[status];
  return field ? meta(metadata, field) : undefined;
}

export function severityOf(status) {
  return SEVERITY[status] ?? 0;
}

/**
 * Is this customer entitled under a LIFETIME (one-time payment) purchase?
 *
 * Lifetime has no Stripe subscription to read a live status from, so the only
 * available signal is our own server-written customer metadata (users cannot
 * edit it — only the secret key can).
 *
 * @param {Record<string,string>|null|undefined} metadata Stripe customer metadata
 */
export function isLifetimeEntitled(metadata) {
  return (
    meta(metadata, 'billing_period') === 'lifetime' &&
    !TERMINAL_STATUSES.includes(meta(metadata, 'status'))
  );
}

/** Is this customer already in a terminal state? */
export function isTerminal(metadata) {
  return TERMINAL_STATUSES.includes(meta(metadata, 'status'));
}

/**
 * Has the money actually gone back (refund) or been clawed back (chargeback)?
 *
 * Deliberately STRICTER than isTerminal: a CANCELLED subscriber paid for their
 * current period and keeps key retrieval until the key's own expiry — that tail
 * is the product policy being sold, not a leak. A refunded or charged-back
 * customer no longer holds a standing payment, so re-delivering their key
 * (recovery mail, session lookup) would undermine the refund itself.
 */
export function isRevoked(metadata) {
  return severityOf(meta(metadata, 'status')) >= SEVERITY.refunded;
}

/**
 * Does this customer still hold a paid charge OTHER than the one being
 * processed — one that succeeded, was not refunded and is not disputed?
 *
 * Terminal status is written on the CUSTOMER, but a refund happens to a
 * CHARGE, and the two are not the same thing. Without this check, refunding
 * any one charge revokes the customer's entire entitlement:
 *
 *   - a buyer double-charged for Signal Lifetime has the duplicate refunded as
 *     support intended, and loses the $299 they legitimately paid;
 *   - a monthly subscriber who upgrades to lifetime and gets the leftover
 *     monthly charge refunded as a courtesy loses the lifetime purchase.
 *
 * Neither recovers on its own — only a fresh `checkout.session.completed`
 * resets the customer to 'active'.
 *
 * Deliberately charge-based rather than keyed off a recorded entitling charge:
 * customers issued before this code shipped have no such record, and a rule
 * that only protects new customers is the wrong half to protect. "Any payment
 * still standing" needs no migration and reads the same for everyone.
 *
 * Fails CLOSED on an API error — if we cannot establish that another payment
 * survives, we do not revoke. Wrongly keeping a refunded customer's access is
 * a support ticket; wrongly revoking a paying customer's is a refund request
 * and a lost customer.
 *
 * @param {{charges:{list:Function}}} stripe
 * @param {string} customerId
 * @param {string|null} excludeChargeId the charge being refunded/disputed
 * @returns {Promise<boolean>}
 */
export async function hasOtherStandingCharge(stripe, customerId, excludeChargeId) {
  try {
    const charges = await stripe.charges.list({ customer: customerId, limit: 100 });
    return (charges.data || []).some(
      (c) =>
        c.id !== excludeChargeId &&
        c.paid === true &&
        c.status === 'succeeded' &&
        c.refunded !== true &&
        c.disputed !== true,
    );
  } catch (err) {
    console.error('charge lookup failed; not revoking', customerId, err?.message);
    return true;
  }
}

/**
 * Build the metadata patch that moves a customer into a terminal status.
 *
 * IDEMPOTENT BY CONSTRUCTION — this is the property that stands in for the
 * event-id dedup store the Pages project has no binding for (see the dispatch
 * comment in functions/api/streets/activate.js):
 *
 *   - re-delivering the SAME event produces a byte-identical patch, because the
 *     first-seen timestamp is preserved rather than refreshed;
 *   - a LESS severe event arriving after a more severe one does not downgrade
 *     `streets_status` (a subscription cancellation that trails a chargeback
 *     must not read as a plain cancellation);
 *   - a genuinely NEW terminal episode — a refund after the customer
 *     re-purchased and was set back to 'active' — does get a fresh timestamp.
 *
 * Returns ONLY the keys to write. Stripe merges metadata on update, so naming
 * fewer keys is both the smallest write and the safest one.
 *
 * @param {Record<string,string>|null|undefined} metadata current customer metadata
 * @param {'cancelled'|'refunded'|'chargeback'} status
 * @param {string} nowIso ISO-8601 timestamp for a first observation
 */
export function terminalStatusPatch(metadata, status, nowIso) {
  const stampKey = STAMP_KEY[status];
  if (!stampKey) throw new Error(`not a terminal status: ${status}`);

  // "Same episode" = the customer is already at or beyond this severity, so
  // this delivery is a repeat or a trailing weaker event, not a new event.
  const sameEpisode = severityOf(meta(metadata, 'status')) >= severityOf(status);
  const existingStamp = readStamp(metadata, status);

  const patch = {};
  patch[stampKey] = sameEpisode && existingStamp ? existingStamp : nowIso;
  if (!sameEpisode) patch[metaKey('status')] = status;
  return patch;
}

/**
 * Unwrap a Stripe reference that may be an id string OR an expanded object.
 * `charge.customer`, `dispute.charge` and friends are one or the other
 * depending on the event and the API version, and reading `.id` off a string
 * (or `.toLowerCase()` off a null) is how these handlers throw TypeErrors.
 */
export function stripeIdOf(ref) {
  if (!ref) return null;
  if (typeof ref === 'string') return ref;
  if (typeof ref === 'object' && typeof ref.id === 'string') return ref.id;
  return null;
}

/**
 * Find or create the Stripe customer an event belongs to.
 *
 * ORDERING IS THE FIX: this used to lower-case `email` BEFORE the caller's
 * `if (!email)` guard ran, so a checkout session with no email raised
 * `TypeError: Cannot read properties of null (reading 'toLowerCase')` and the
 * webhook answered a generic 500 — which Stripe then retried, repeatedly, with
 * nothing in the log naming the actual problem. Guard first, normalise second.
 *
 * @param {{customers:{list:Function,create:Function}}} stripe Stripe client
 */
export async function resolveCustomerId(stripe, customerId, email) {
  if (customerId) return customerId;
  if (!email) {
    throw new Error('cannot resolve a Stripe customer: no customer id and no email on the event');
  }
  const normalized = email.toLowerCase();
  const existing = await stripe.customers.list({ email: normalized, limit: 1 });
  if (existing.data.length > 0) return existing.data[0].id;
  const created = await stripe.customers.create({ email: normalized });
  return created.id;
}
