// Cloudflare Pages Function: License Lease Refresh (the heart of the lease model)
//
// POST /api/license/refresh  { "key": "4DA-LIC-...", "fingerprint"?: "..." }
//
// STATELESS by design — Stripe is the ONLY source of truth. Given a stable refresh
// credential, we look up the customer, read their LIVE subscription status, and if
// entitled, mint a short-lived (TOKEN_TTL_DAYS) Ed25519 entitlement token the
// desktop app verifies OFFLINE. Revocation is automatic: cancel/refund in Stripe
// => the live status flips => the next refresh denies. No revocation state to keep
// in sync, no database, infinite horizontal scale.
//
// THE LIMIT OF THAT CLAIM: "the next refresh denies" only bounds access for a
// client that actually refreshes. A LIFETIME licence key issued by
// /api/license/activate carries an embedded expiry of the year 2099 and verifies
// offline against the app's built-in public key, so a refunded lifetime holder who
// simply never calls this endpoint keeps working regardless of what Stripe says.
// This endpoint is only load-bearing for revocation once every tier is issued a
// short-dated key. See the terminal-status note in functions/api/license/activate.js.
//
// Secrets: STRIPE_SECRET_KEY, LICENSE_PRIVATE_KEY_HEX.

import Stripe from 'stripe';
import { signLicenseToken } from '../../../lib/ed25519-license.js';
import { isLifetimeEntitled, meta } from '../../../lib/entitlement.js';

// Lease window. Aligned with the app's 30-day activation grace so an offline
// user's token never expires *before* their grace does (avoids a confusing
// "expired" badge while still entitled). Online apps refresh every 6h, so
// revocation latency is bounded by the refresh interval, not this TTL.
const TOKEN_TTL_DAYS = 30;
const ENTITLING_SUB_STATUSES = ['active', 'trialing', 'past_due']; // past_due = dunning grace

function normalizeTier(tier) {
  if (tier === 'pro' || tier === 'community' || tier === 'cohort') return 'signal';
  return tier || 'signal';
}

function json(body, status) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

export async function onRequest({ request, env }) {
  if (request.method === 'OPTIONS') {
    return new Response(null, {
      status: 200,
      headers: {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'POST, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type',
      },
    });
  }
  if (request.method !== 'POST') return json({ valid: false, reason: 'method_not_allowed' }, 405);

  if (!env.STRIPE_SECRET_KEY || !env.LICENSE_PRIVATE_KEY_HEX) {
    return json({ valid: false, reason: 'service_not_configured' }, 500);
  }

  let key;
  try {
    const body = await request.json();
    key = typeof body?.key === 'string' ? body.key.trim() : '';
  } catch {
    return json({ valid: false, reason: 'invalid_body' }, 400);
  }

  // Format guard: refresh credentials are 4DA-LIC-<base32>. Never accept a signed
  // token here, and never let arbitrary strings reach the Stripe search query.
  if (!/^4DA-LIC-[A-Z2-7]{16,80}$/.test(key)) {
    return json({ valid: false, reason: 'invalid_key_format' }, 400);
  }

  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });

    // Look up the customer by the refresh credential (server-written metadata).
    const found = await stripe.customers.search({
      query: `metadata['refresh_key']:'${key}'`,
      limit: 1,
    });
    if (!found.data.length) {
      return json({ valid: false, reason: 'not_found' }, 403);
    }
    const customer = found.data[0];
    if (customer.deleted) return json({ valid: false, reason: 'not_found' }, 403);

    // Entitlement is read LIVE from Stripe, never trusted from stale metadata.
    const subs = await stripe.subscriptions.list({ customer: customer.id, status: 'all', limit: 20 });
    const activeSub = subs.data.find((s) => ENTITLING_SUB_STATUSES.includes(s.status));

    // Lifetime (one-time payment) has no subscription; entitlement is our own
    // server-written metadata (users cannot edit customer metadata — only the
    // secret key can), and only while not cancelled/refunded/charged back.
    //
    // The terminal-status list deliberately lives in lib/entitlement.js and is
    // shared with the webhook that WRITES it. This check used to be spelled out
    // inline as `!== 'cancelled' && !== 'refunded'` — and 'refunded' was never
    // written by anything, anywhere in the repo, because there was no refund
    // handler. A reader testing for a value no writer produced reads exactly
    // like a working revocation check and is not one.
    const isLifetime = isLifetimeEntitled(customer.metadata);

    if (!activeSub && !isLifetime) {
      return json({ valid: false, reason: 'no_active_entitlement' }, 403);
    }

    const tier = normalizeTier(
      meta(activeSub?.metadata, 'tier') || meta(customer.metadata, 'tier') || 'signal',
    );

    const now = new Date();
    const expiresAt = new Date(now.getTime() + TOKEN_TTL_DAYS * 24 * 60 * 60 * 1000);
    const payload = {
      tier,
      email: customer.email || '',
      expires_at: expiresAt.toISOString(),
      issued_at: now.toISOString(),
      features: [tier],
      license_id: customer.id, // opaque ref; verifier ignores unknown fields
    };

    const token = await signLicenseToken(payload, env.LICENSE_PRIVATE_KEY_HEX);

    return json(
      { valid: true, token, tier, expires_at: expiresAt.toISOString(), lease_days: TOKEN_TTL_DAYS },
      200,
    );
  } catch (err) {
    console.error('License refresh failed:', err?.message);
    // Do NOT deny on a server/Stripe error — the client keeps its current token
    // (offline-tolerant). Signal a retryable error, not a revocation.
    return json({ valid: false, reason: 'temporary_error', retryable: true }, 503);
  }
}
