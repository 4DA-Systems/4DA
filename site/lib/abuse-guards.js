// KV-backed abuse guards for the licensing endpoints.
//
// Two jobs, both running against the LICENSE_KV namespace bound in
// site/wrangler.toml (created 2026-08-19; prod 33eb6985..., preview 83ffb6f2...):
//
//   1. WEBHOOK EVENT DEDUP — Stripe retries a delivery until it gets a 2xx and
//      routinely re-sends the same event id. The terminal handlers in
//      functions/api/license/activate.js are idempotent by construction, but
//      `checkout.session.completed` and `invoice.paid` MINT a new signed licence
//      key on every delivery, so a duplicate used to issue a second valid key
//      with a fresh expiry window. Minting cannot be made idempotent by
//      construction; it needs exactly this: a persisted record of processed
//      event ids, checked before dispatch and written after success.
//
//   2. RECOVERY RATE LIMITING — GET ?email= recovery was unauthenticated and
//      unmetered. It could never mail a non-customer, but ~100 requests for ONE
//      known customer address exhausts the Resend free tier's 100/day quota,
//      which silences EVERY licence email for the day — including the delivery
//      mail of every real purchase. That upgrades "inbox nuisance" to a
//      denial-of-licence-delivery, which is why the caps exist.
//
// FAIL-OPEN CONTRACT (every function here): a KV read/write error must never
// block the live payment path or a legitimate recovery. On error we log and
// behave as if the guard passed. The guards bound abuse; they are not
// load-bearing for correctness.
//
// NON-ATOMICITY, acknowledged: KV has no atomic increment and is eventually
// consistent (~60s), so concurrent callers can slightly exceed a cap and a
// duplicate delivered within seconds of the first can slip the dedup check.
// Stripe's retry schedule is minutes-to-hours apart, and the rate caps bound
// sustained abuse, so approximate counting is the right trade here.
//
// DELIBERATELY DEPENDENCY-FREE, like entitlement.js: site/ has no installed
// dependency tree in CI, so only import-free modules can carry regression tests
// (abuse-guards.test.mjs, wired into `pnpm run test:scripts`).

// ---------------------------------------------------------------------------
// 1. Webhook event dedup
// ---------------------------------------------------------------------------

/** Comfortably longer than Stripe's ~3-day live-mode retry window. */
export const EVENT_DEDUP_TTL_SECONDS = 7 * 24 * 60 * 60;

export function eventKey(eventId) {
  return `evt:${eventId}`;
}

/**
 * Has this Stripe event id already been fully processed?
 * Fails OPEN: a KV error reads as "not a duplicate" so the event is processed.
 */
export async function isDuplicateEvent(kv, eventId) {
  try {
    return (await kv.get(eventKey(eventId))) !== null;
  } catch (err) {
    console.error('KV dedup read failed (failing open):', err?.message);
    return false;
  }
}

/**
 * Record an event id as processed. Called only AFTER the handler succeeded —
 * a failed handler must stay retryable, so its id is never recorded.
 * A write failure is logged and swallowed: losing one dedup record only means
 * one delivery could be processed twice, which is yesterday's status quo.
 */
export async function markEventProcessed(kv, eventId) {
  try {
    await kv.put(eventKey(eventId), new Date().toISOString(), {
      expirationTtl: EVENT_DEDUP_TTL_SECONDS,
    });
  } catch (err) {
    console.error('KV dedup write failed (non-fatal):', err?.message);
  }
}

// ---------------------------------------------------------------------------
// 2. Recovery rate limiting (fixed windows)
// ---------------------------------------------------------------------------

/**
 * Recovery mails per ADDRESS per UTC day. Generous for a human who lost an
 * email (they need 1), tight against quota-burning: one hostile loop can cost
 * at most this many of the day's Resend sends per known address.
 */
export const RECOVERY_MAILS_PER_ADDRESS_PER_DAY = 5;

/**
 * Recovery REQUESTS per IP per UTC hour. Bounds scripted probing regardless of
 * how many addresses are tried. High enough that a support person retrying for
 * a customer never sees it.
 */
export const RECOVERY_REQUESTS_PER_IP_PER_HOUR = 30;

/** Window TTLs: twice the window length so a live window can never expire early. */
export const MAIL_WINDOW_TTL_SECONDS = 2 * 24 * 60 * 60;
export const IP_WINDOW_TTL_SECONDS = 2 * 60 * 60;

/** `rl:mail:<address>:<yyyy-mm-dd>` — address normalised so casing can't dodge the cap. */
export function mailWindowKey(email, now = new Date()) {
  return `rl:mail:${String(email).trim().toLowerCase()}:${now.toISOString().slice(0, 10)}`;
}

/** `rl:ip:<ip>:<yyyy-mm-ddThh>` */
export function ipWindowKey(ip, now = new Date()) {
  return `rl:ip:${ip}:${now.toISOString().slice(0, 13)}`;
}

/**
 * Newsletter/notify signups per IP per UTC hour. /api/notify creates or updates
 * a Stripe CUSTOMER on every call with no auth — unmetered, that is an
 * unbounded write into the Stripe customer namespace (list pollution, and
 * recovery/session lookups take the newest customer for an email). Separate key
 * namespace from `rl:ip` so a signup and a recovery from the same IP do not
 * share one counter.
 */
export const NOTIFY_REQUESTS_PER_IP_PER_HOUR = 20;

/** `rl:notify:<ip>:<yyyy-mm-ddThh>` */
export function notifyWindowKey(ip, now = new Date()) {
  return `rl:notify:${ip}:${now.toISOString().slice(0, 13)}`;
}

/**
 * Check a fixed-window counter and, if under the limit, count this call.
 * Returns true when the call is allowed. Fails OPEN on any KV error.
 */
export async function checkAndCount(kv, key, limit, ttlSeconds) {
  try {
    const current = parseInt((await kv.get(key)) || '0', 10) || 0;
    if (current >= limit) return false;
    await kv.put(key, String(current + 1), { expirationTtl: ttlSeconds });
    return true;
  } catch (err) {
    console.error('KV rate-limit failed (failing open):', err?.message);
    return true;
  }
}
