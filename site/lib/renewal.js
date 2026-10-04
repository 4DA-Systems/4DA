// Silent licence renewal — the decisions, separated from the I/O.
//
// THE PROBLEM THIS SOLVES
// -----------------------
// A subscription key is a signed, offline-verifiable token with an expiry inside
// it (monthly ~35 days, annual ~1 year + 7 days). Each renewal used to reach the
// customer only as an EMAIL with a new key that they had to click or paste. A
// subscriber who ignored that mail was dropped to Free while still being billed —
// every month, for every monthly customer. Terms §4.3 has always said the app
// "contacts our licensing endpoint with only your license key" when refreshing a
// licence; this is the code that finally does it.
//
// THE MODEL
// ---------
// The app presents the key it already holds. That key is its own credential: it
// is Ed25519-signed by us, so it cannot be forged, and it names the purchase
// email. We look up that customer's LIVE subscription in Stripe and return a key
// valid to the end of the period they have paid for, plus grace. Nothing is
// stored on our side; Stripe stays the only source of truth, which is what lets
// this scale without a database. Revocation falls out for free: a cancelled
// subscription simply stops being renewed and the key lapses at period end; a
// refunded or charged-back customer is never renewed.
//
// DELIBERATELY DEPENDENCY-FREE (like entitlement.js) so test:scripts can run it.
// The signature check itself lives in the Pages Function, which can import
// @noble/ed25519; this module only parses and decides.

/** Days of access past the paid period end, so a renewal that lands a little
 * late (Stripe retry, an app that was offline) never interrupts a payer. */
export const RENEWAL_GRACE_DAYS = 7;

// A key of ANY age is a valid renewal credential while its subscription is
// live. There used to be a 60-day lapse bound "so an ancient leaked key is not a
// perpetual credential" — but every key for an email carries exactly the same
// power (the live Stripe subscription is the only gate), so an old leaked key
// is no worse than last month's. The bound only stranded honest customers: the
// key in their PURCHASE email stopped working on a new computer, which is what
// forced a fresh key into their inbox every month. The app still stops asking
// on its own 60 days after expiry; that bounds traffic, not entitlement.

/** Stripe subscription statuses that still entitle. past_due = Stripe is
 * retrying a failed card (dunning); the customer keeps access meanwhile, but
 * only on a short lease — see renewedExpiryFor. */
export const ENTITLING_SUB_STATUSES = ['active', 'trialing', 'past_due'];

/** During dunning a key runs at most this far ahead of now. Stripe advances the
 * period the moment it TRIES to charge, so the period end is not proof of
 * payment while the card is failing; handing out a key to it would give every
 * failed card a free month. The app renews every 12h inside this window, so a
 * customer who fixes their card never notices; one whose card never recovers
 * lapses within a week of Stripe giving up. */
export const PAST_DUE_LEASE_DAYS = 7;

const DAY_MS = 24 * 60 * 60 * 1000;
const KEY_RE = /^4DA-([A-Za-z0-9+/]+={0,2})\.([A-Za-z0-9+/]+={0,2})$/;

function b64ToBytes(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * Split a presented `4DA-{payload}.{sig}` key into its parts.
 * Returns null for anything that is not exactly that shape — including the
 * `4DA-LIC-` refresh credentials, which have no `.`.
 *
 * @returns {{payloadBytes: Uint8Array, sigBytes: Uint8Array, payload: object} | null}
 */
export function parseLicenseKey(key) {
  if (typeof key !== 'string' || key.length > 1024) return null;
  const m = KEY_RE.exec(key.replace(/\s+/g, ''));
  if (!m) return null;
  try {
    const payloadBytes = b64ToBytes(m[1]);
    const sigBytes = b64ToBytes(m[2]);
    if (sigBytes.length !== 64) return null;
    const payload = JSON.parse(new TextDecoder().decode(payloadBytes));
    if (!payload || typeof payload.email !== 'string' || !payload.email || typeof payload.expires_at !== 'string') {
      return null;
    }
    return { payloadBytes, sigBytes, payload };
  } catch {
    return null;
  }
}

/**
 * What kind of key is this, before we touch Stripe?
 * @returns {'lifetime'|'eligible'|'invalid'}
 */
export function classifyPresentedKey(payload) {
  const exp = Date.parse(payload?.expires_at);
  if (Number.isNaN(exp)) return 'invalid';
  if (new Date(exp).getUTCFullYear() >= 2099) return 'lifetime';
  return 'eligible';
}

/**
 * When does this subscription's paid period end? Handles both API shapes:
 * basil+ moved `current_period_end` from the subscription onto its items.
 * @returns {number|null} UNIX seconds
 */
export function subscriptionPeriodEnd(sub) {
  const fromItems = (sub?.items?.data || [])
    .map((i) => i?.current_period_end)
    .filter((n) => typeof n === 'number');
  if (fromItems.length) return Math.max(...fromItems);
  return typeof sub?.current_period_end === 'number' ? sub.current_period_end : null;
}

/** The entitling subscription with the latest paid period end, or null. */
export function pickEntitlingSubscription(subs) {
  let best = null;
  let bestEnd = -1;
  for (const s of subs || []) {
    if (!ENTITLING_SUB_STATUSES.includes(s?.status)) continue;
    const end = subscriptionPeriodEnd(s);
    if (end !== null && end > bestEnd) {
      best = s;
      bestEnd = end;
    }
  }
  return best;
}

/** Expiry for a renewed key: paid period end + grace. */
export function renewedExpiry(periodEndSeconds) {
  return new Date(periodEndSeconds * 1000 + RENEWAL_GRACE_DAYS * DAY_MS);
}

/** Expiry for a renewed key given the entitling subscription itself: the paid
 * period end + grace, capped to a short lease while the card is failing. */
export function renewedExpiryFor(sub, nowMs) {
  const full = renewedExpiry(subscriptionPeriodEnd(sub));
  if (sub?.status !== 'past_due') return full;
  const lease = new Date(nowMs + PAST_DUE_LEASE_DAYS * DAY_MS);
  return lease < full ? lease : full;
}

/**
 * Is a key that expires at `candidateExpiry` worth handing back to a client that
 * already holds one expiring at `presentedExpiry`? Only if it actually extends
 * access — otherwise the client is told it is current and keeps what it has.
 */
export function extendsAccess(presentedExpiryIso, candidateExpiry) {
  const presented = Date.parse(presentedExpiryIso);
  const candidate = candidateExpiry instanceof Date ? candidateExpiry.getTime() : Date.parse(candidateExpiry);
  if (Number.isNaN(candidate)) return false;
  if (Number.isNaN(presented)) return true;
  return candidate > presented + 60 * 60 * 1000; // > 1h later
}
