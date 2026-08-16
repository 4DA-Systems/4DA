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

/**
 * Severity ranking. Webhook deliveries are NOT ordered — Stripe can deliver
 * `customer.subscription.deleted` after `charge.dispute.created` for the same
 * customer — so the write must converge on the most severe outcome regardless
 * of arrival order rather than letting the last message win.
 */
const SEVERITY = { active: 0, cancelled: 1, refunded: 2, chargeback: 3 };

/** Where each terminal status records WHEN it was first observed. */
const STAMP_KEY = {
  cancelled: 'streets_cancelled_at',
  refunded: 'streets_refunded_at',
  chargeback: 'streets_chargeback_at',
};

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
  const md = metadata || {};
  return md.streets_billing_period === 'lifetime' && !TERMINAL_STATUSES.includes(md.streets_status);
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

  const md = metadata || {};
  // "Same episode" = the customer is already at or beyond this severity, so
  // this delivery is a repeat or a trailing weaker event, not a new event.
  const sameEpisode = severityOf(md.streets_status) >= severityOf(status);

  const patch = {};
  patch[stampKey] = sameEpisode && md[stampKey] ? md[stampKey] : nowIso;
  if (!sameEpisode) patch.streets_status = status;
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
