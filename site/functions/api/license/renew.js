// Cloudflare Pages Function: silent licence renewal for Signal subscribers.
//
// POST /api/license/renew   { "key": "4DA-<payload>.<sig>" }
//
// The desktop app calls this ONLY for a subscription key that is near or just
// past its expiry, and sends ONLY that key (Terms §4.3, NETWORK.md §2k). The key
// is its own credential: we signed it, so it cannot be forged, and it names the
// purchase email. We confirm the LIVE subscription in Stripe and answer with a
// key valid to the end of the period that has actually been paid for, plus
// grace. Stateless — Stripe is the only source of truth — so it scales with no
// database and needs no revocation list: cancelled subscriptions stop renewing
// and lapse at period end; refunded / charged-back customers never renew.
//
// Responses (always JSON, never cached):
//   200 {status:'renewed', license_key, expires_at}  — install this key
//   200 {status:'current'}                           — what you hold is newest
//   200 {status:'lifetime'}                          — lifetime keys never renew
//   403 {status:'not_entitled'|'lapsed'}             — keep your key; it will lapse
//   400 / 401                                        — malformed / not our signature
//   429 / 503 {retryable:true}                       — try again later, keep your key
//
// Secrets: STRIPE_SECRET_KEY, LICENSE_PRIVATE_KEY_HEX. KV: LICENSE_KV (optional).

import Stripe from 'stripe';
import * as ed from '@noble/ed25519';
import { hexToBytes, signLicenseToken } from '../../../lib/ed25519-license.js'; // also wires SHA-512
import { APP_LICENSE_PUBLIC_KEY_HEX } from '../../../lib/license-health.js';
import { isRevoked, meta, metaKey } from '../../../lib/entitlement.js';
import { checkAndCount } from '../../../lib/abuse-guards.js';
import {
  classifyPresentedKey,
  extendsAccess,
  parseLicenseKey,
  pickEntitlingSubscription,
  renewedExpiry,
  subscriptionPeriodEnd,
} from '../../../lib/renewal.js';

// Generous: a healthy client asks at most every 12h, and only near expiry.
const RENEWALS_PER_KEY_PER_DAY = 24;
const RENEW_WINDOW_TTL_SECONDS = 2 * 24 * 60 * 60;

function json(body, status) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

async function keyFingerprint(key) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(key));
  return Array.from(new Uint8Array(digest).slice(0, 12), (b) => b.toString(16).padStart(2, '0')).join('');
}

async function signatureIsOurs(parsed) {
  try {
    return await ed.verifyAsync(parsed.sigBytes, parsed.payloadBytes, hexToBytes(APP_LICENSE_PUBLIC_KEY_HEX));
  } catch {
    return false;
  }
}

/** The customer (for this email) holding the best entitling subscription. */
async function findEntitledCustomer(stripe, email) {
  const emails = email === email.toLowerCase() ? [email] : [email, email.toLowerCase()];
  let best = null;
  for (const address of emails) {
    const customers = await stripe.customers.list({ email: address, limit: 10 });
    for (const customer of customers.data || []) {
      if (customer.deleted || isRevoked(customer.metadata)) continue;
      const subs = await stripe.subscriptions.list({ customer: customer.id, status: 'all', limit: 20 });
      const sub = pickEntitlingSubscription(subs.data);
      const end = sub ? subscriptionPeriodEnd(sub) : null;
      if (end !== null && (!best || end > best.periodEnd)) best = { customer, periodEnd: end };
    }
    if (best) break;
  }
  return best;
}

/** A stored key on the customer that already covers `expiry`, if any — reusing
 * it avoids minting a fresh key every time a client asks. */
async function reusableStoredKey(customer, email, expiry) {
  const stored = meta(customer.metadata, 'license');
  const parsed = stored ? parseLicenseKey(stored) : null;
  if (!parsed || parsed.payload.email !== email) return null;
  if (Date.parse(parsed.payload.expires_at) < expiry.getTime() - 60 * 60 * 1000) return null;
  return (await signatureIsOurs(parsed)) ? stored : null;
}

async function issue(env, stripe, customer, presented, expiry) {
  const stored = await reusableStoredKey(customer, presented.email, expiry);
  if (stored) return { key: stored, expiresAt: meta(customer.metadata, 'expires_at') || expiry.toISOString() };

  const now = new Date();
  const tier = meta(customer.metadata, 'tier') || presented.tier || 'signal';
  // Same field order as activate.js generateAndStoreLicense — the signed bytes
  // are JSON.stringify(payload), and the app parses exactly these fields.
  const payload = {
    tier,
    email: presented.email,
    expires_at: expiry.toISOString(),
    issued_at: now.toISOString(),
    features: [tier],
  };
  const key = await signLicenseToken(payload, env.LICENSE_PRIVATE_KEY_HEX);
  // Keep the customer record current so email recovery hands out this key too.
  // Status is deliberately NOT written: entitlement was just confirmed live, and
  // terminal statuses are owned by the webhook handlers.
  await stripe.customers.update(customer.id, {
    metadata: {
      [metaKey('license')]: key,
      [metaKey('expires_at')]: payload.expires_at,
      [metaKey('issued_at')]: payload.issued_at,
    },
  });
  return { key, expiresAt: payload.expires_at };
}

export async function onRequest({ request, env }) {
  if (request.method !== 'POST') return json({ error: 'Method not allowed' }, 405);
  if (!env.STRIPE_SECRET_KEY || !env.LICENSE_PRIVATE_KEY_HEX) {
    return json({ status: 'error', reason: 'service_not_configured', retryable: true }, 503);
  }

  let presentedKey;
  try {
    presentedKey = String((await request.json())?.key || '').replace(/\s+/g, '');
  } catch {
    return json({ status: 'error', reason: 'invalid_body' }, 400);
  }
  const parsed = parseLicenseKey(presentedKey);
  if (!parsed) return json({ status: 'error', reason: 'invalid_key_format' }, 400);
  if (!(await signatureIsOurs(parsed))) return json({ status: 'error', reason: 'invalid_signature' }, 401);

  const presented = parsed.payload;
  const kind = classifyPresentedKey(presented, Date.now());
  if (kind === 'lifetime') return json({ status: 'lifetime' }, 200);
  if (kind === 'lapsed') return json({ status: 'lapsed' }, 403);
  if (kind !== 'eligible') return json({ status: 'error', reason: 'invalid_key_format' }, 400);

  if (env.LICENSE_KV) {
    const allowed = await checkAndCount(
      env.LICENSE_KV,
      `renew:${await keyFingerprint(presentedKey)}:${new Date().toISOString().slice(0, 10)}`,
      RENEWALS_PER_KEY_PER_DAY,
      RENEW_WINDOW_TTL_SECONDS,
    );
    if (!allowed) return json({ status: 'error', reason: 'rate_limited', retryable: true }, 429);
  }

  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });
    const entitled = await findEntitledCustomer(stripe, presented.email);
    if (!entitled) return json({ status: 'not_entitled' }, 403);

    const expiry = renewedExpiry(entitled.periodEnd);
    if (!extendsAccess(presented.expires_at, expiry)) return json({ status: 'current' }, 200);

    const { key, expiresAt } = await issue(env, stripe, entitled.customer, presented, expiry);
    console.log('Licence renewed silently for customer', entitled.customer.id, 'until', expiresAt);
    return json({ status: 'renewed', license_key: key, expires_at: expiresAt }, 200);
  } catch (err) {
    // Never deny on our own failure: the client keeps its current key.
    console.error('Licence renewal failed:', err?.message);
    return json({ status: 'error', reason: 'temporary_error', retryable: true }, 503);
  }
}
