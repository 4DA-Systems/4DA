// Cloudflare Pages Function: Signal License Activation
//
// NOTE ON THE PATH: this lives at /api/streets/activate for historical reasons
// (the paid tier used to be branded "STREETS"). It is the SIGNAL license endpoint.
// The path is kept identical to the old Vercel route so the Stripe webhook config
// and the desktop app's hardcoded recovery URL keep working unchanged.
//
// POST: Stripe webhook — handles:
//   - checkout.session.completed   -> initial license generation
//   - invoice.paid                 -> subscription renewal (fresh key + extended expiry)
//   - customer.subscription.deleted -> cancellation (mark metadata)
// GET:  Retrieve license by checkout session_id (returns the key — the session id
//       proves the purchase) or recover by email (MAILS the key to the address on
//       file; never returns it). See the GET block for the security rationale.
//
// Secrets/vars (Cloudflare Pages -> Settings -> Environment variables):
//   STRIPE_SECRET_KEY       — Stripe secret key
//   STRIPE_WEBHOOK_SECRET   — Stripe webhook signing secret
//   LICENSE_PRIVATE_KEY_HEX — Ed25519 private key seed (hex, 64 chars) for signing keys
//   RESEND_API_KEY          — Resend API key; REQUIRED for email-based recovery
//   RESEND_FROM_EMAIL       — e.g. "4DA <licenses@4da.ai>"; REQUIRED for email-based recovery
//   ENVIRONMENT             — "production" in prod; anything else enables localhost CORS

import Stripe from 'stripe';
import * as ed from '@noble/ed25519';
import { generateRefreshKey } from '../../../lib/ed25519-license.js';
import {
  deliverRecoveryEmail,
  isPlausibleEmail,
  isRecoveryEmailConfigured,
} from '../../../lib/recovery-email.js';

// ---------------------------------------------------------------------------
// Ed25519 on the Workers runtime.
//
// The original Vercel handler used Node's `crypto` (createPrivateKey + sign).
// On Cloudflare's Workers runtime we use @noble/ed25519 (pure JS, RFC 8032 pure
// Ed25519 — byte-identical signatures to Node crypto and Rust ed25519_dalek for
// the same 32-byte seed + message; proven by scripts/verify-ed25519-equivalence.mjs).
//
// @noble/ed25519 v2's async signing needs a SHA-512 implementation; wire it to
// the runtime's WebCrypto (available on both Workers and Node 20+).
// ---------------------------------------------------------------------------

ed.etc.sha512Async = async (...msgs) => {
  let total = 0;
  for (const m of msgs) total += m.length;
  const data = new Uint8Array(total);
  let offset = 0;
  for (const m of msgs) {
    data.set(m, offset);
    offset += m.length;
  }
  return new Uint8Array(await crypto.subtle.digest('SHA-512', data));
};

function hexToBytes(hex) {
  const clean = hex.trim();
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substr(i * 2, 2), 16);
  }
  return bytes;
}

function bytesToB64(bytes) {
  let bin = '';
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

async function signLicenseKey(env, payload) {
  const privHex = env.LICENSE_PRIVATE_KEY_HEX;
  if (!privHex) throw new Error('LICENSE_PRIVATE_KEY_HEX not configured');

  const payloadJson = JSON.stringify(payload);
  const payloadBytes = new TextEncoder().encode(payloadJson);
  const payloadB64 = bytesToB64(payloadBytes);

  const seed = hexToBytes(privHex);
  const signature = await ed.signAsync(payloadBytes, seed); // 64-byte Uint8Array
  const sigB64 = bytesToB64(signature);

  return `4DA-${payloadB64}.${sigB64}`;
}

// ---------------------------------------------------------------------------
// CORS — scope to known origins
// ---------------------------------------------------------------------------

const BASE_ORIGINS = ['https://4da.ai', 'https://www.4da.ai', 'https://streets.4da.ai', 'tauri://localhost'];

function corsHeaders(request, env) {
  const headers = new Headers();
  const origin = request.headers.get('origin');
  const allowed =
    env.ENVIRONMENT !== 'production'
      ? [...BASE_ORIGINS, 'http://localhost:4444', 'http://localhost:1420']
      : BASE_ORIGINS;
  if (origin && allowed.includes(origin)) {
    headers.set('Access-Control-Allow-Origin', origin);
    headers.set('Vary', 'Origin');
  }
  headers.set('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  headers.set('Access-Control-Allow-Headers', 'Content-Type, Stripe-Signature');
  return headers;
}

function json(body, status, headers) {
  headers.set('Content-Type', 'application/json');
  // The session_id path returns a licence key. Never let a browser, proxy or CDN
  // hold a copy of any response from this endpoint.
  headers.set('Cache-Control', 'no-store');
  return new Response(JSON.stringify(body), { status, headers });
}

// ---------------------------------------------------------------------------
// Shared: generate + store license for a customer
//
// Expiry now matches the billing period (fixes the prior bug where MONTHLY subs
// were issued 1-year keys — an ~11-month access leak on cancellation):
//   - monthly  -> ~35 days (covers the month + grace; invoice.paid re-issues each cycle)
//   - annual   -> 1 year + 7 days grace
//   - lifetime -> year 2099
// The billing period is persisted in customer metadata so renewals preserve it.
// ---------------------------------------------------------------------------

async function generateAndStoreLicense(env, stripe, customerId, email, tier, billingPeriod) {
  // Legacy tiers all map to signal
  const effectiveTier = tier === 'community' || tier === 'cohort' || tier === 'pro' ? 'signal' : tier;
  const features = [effectiveTier];

  // Safe default for legacy customers whose metadata predates billing_period:
  // 'annual' (longer) avoids accidentally cutting off a paying subscriber.
  const period = billingPeriod || 'annual';

  const now = new Date();
  const expiresAt = new Date(now);
  if (period === 'lifetime') {
    expiresAt.setFullYear(2099);
  } else if (period === 'monthly') {
    expiresAt.setDate(expiresAt.getDate() + 35);
  } else {
    // annual
    expiresAt.setFullYear(expiresAt.getFullYear() + 1);
    expiresAt.setDate(expiresAt.getDate() + 7);
  }

  const payload = {
    tier: effectiveTier,
    email,
    expires_at: expiresAt.toISOString(),
    issued_at: now.toISOString(),
    features,
  };

  const licenseKey = await signLicenseKey(env, payload);

  // Guard: Stripe metadata values max 500 chars
  if (licenseKey.length > 500) {
    throw new Error(`License key exceeds Stripe metadata limit: ${licenseKey.length} chars`);
  }

  await stripe.customers.update(customerId, {
    metadata: {
      streets_license: licenseKey,
      streets_tier: effectiveTier,
      streets_billing_period: period,
      streets_issued_at: now.toISOString(),
      streets_expires_at: expiresAt.toISOString(),
      streets_status: 'active',
    },
  });

  return { licenseKey, expiresAt };
}

// ---------------------------------------------------------------------------
// Shared: resolve customer ID (find or create)
// ---------------------------------------------------------------------------

async function resolveCustomerId(stripe, customerId, email) {
  if (customerId) return customerId;

  const existing = await stripe.customers.list({ email: email.toLowerCase(), limit: 1 });
  if (existing.data.length > 0) return existing.data[0].id;

  const created = await stripe.customers.create({ email: email.toLowerCase() });
  return created.id;
}

// Lease model: ensure the customer has a STABLE, unguessable refresh credential
// stored in metadata (generated once, reused forever). The desktop lease client
// presents this to /api/license/refresh to mint short-lived entitlement tokens.
// Idempotent — never regenerates an existing key (that would break the user's
// stored credential). Additive: the legacy long-token flow is untouched.
async function ensureRefreshKey(stripe, customerId) {
  try {
    const c = await stripe.customers.retrieve(customerId);
    if (c.deleted) return null;
    if (c.metadata?.refresh_key) return c.metadata.refresh_key;
    const key = generateRefreshKey();
    await stripe.customers.update(customerId, { metadata: { refresh_key: key } });
    return key;
  } catch (err) {
    // Non-fatal: the legacy token was already issued; refresh_key can be
    // back-filled on the next event. Never fail the webhook over this.
    console.error('ensureRefreshKey failed (non-fatal):', err?.message);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Webhook event handlers
// ---------------------------------------------------------------------------

const HANDLED_EVENTS = ['checkout.session.completed', 'invoice.paid', 'customer.subscription.deleted'];

async function handleCheckoutCompleted(env, stripe, session) {
  const email = session.customer_email || session.customer_details?.email;
  const customerId = await resolveCustomerId(stripe, session.customer, email);
  const tier = session.metadata?.streets_tier || 'signal';
  const billingPeriod = session.metadata?.billing_period;

  if (!email) {
    throw new Error(`No customer email in session ${session.id}`);
  }

  const { licenseKey } = await generateAndStoreLicense(env, stripe, customerId, email, tier, billingPeriod);
  // Lease model: back the account with a stable refresh credential for the
  // short-lived-token flow (additive; legacy long token above still delivered).
  const refreshKey = await ensureRefreshKey(stripe, customerId);
  console.log('License generated:', email, 'tier:', tier, 'period:', billingPeriod, 'customer:', customerId, 'len:', licenseKey.length, 'refresh_key:', refreshKey ? 'set' : 'none');
  return { license_generated: true };
}

async function handleInvoicePaid(env, stripe, invoice) {
  // Only process subscription invoices (not one-time payments)
  if (!invoice.subscription) {
    return { skipped: 'not a subscription invoice' };
  }

  // Skip the initial invoice — checkout.session.completed handles that
  if (invoice.billing_reason === 'subscription_create') {
    return { skipped: 'initial invoice handled by checkout.session.completed' };
  }

  const customerId = invoice.customer;
  if (!customerId) {
    throw new Error('No customer ID on invoice');
  }

  const customer = await stripe.customers.retrieve(customerId);
  const email = customer.email;
  const existingTier = customer.metadata?.streets_tier || 'signal';
  // Preserve the billing period across renewals so annual keys stay annual.
  const billingPeriod = customer.metadata?.streets_billing_period;

  if (!email) {
    throw new Error(`No email for customer ${customerId}`);
  }

  // Regenerate license with fresh expiry
  const { licenseKey } = await generateAndStoreLicense(env, stripe, customerId, email, existingTier, billingPeriod);
  console.log('License renewed:', email, 'tier:', existingTier, 'period:', billingPeriod, 'customer:', customerId, 'reason:', invoice.billing_reason, 'len:', licenseKey.length);
  return { license_renewed: true };
}

async function handleSubscriptionDeleted(stripe, subscription) {
  const customerId = subscription.customer;
  if (!customerId) {
    return { skipped: 'no customer ID' };
  }

  const customer = await stripe.customers.retrieve(customerId);

  // Don't revoke immediately — the existing license key is still valid until
  // its embedded expires_at date. Just mark the status so the app can show a
  // "subscription cancelled" message and the GET endpoint can inform the user.
  await stripe.customers.update(customerId, {
    metadata: {
      ...customer.metadata,
      streets_status: 'cancelled',
      streets_cancelled_at: new Date().toISOString(),
    },
  });

  console.log('Subscription cancelled:', customer.email, 'customer:', customerId);
  return { subscription_cancelled: true };
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

export async function onRequest({ request, env, waitUntil }) {
  const headers = corsHeaders(request, env);

  if (request.method === 'OPTIONS') return new Response(null, { status: 200, headers });

  // -------------------------------------------------------------------------
  // POST: Stripe webhook
  // -------------------------------------------------------------------------
  if (request.method === 'POST') {
    const webhookSecret = env.STRIPE_WEBHOOK_SECRET;
    if (!webhookSecret) {
      return json({ error: 'Webhook secret not configured' }, 500, headers);
    }

    // Raw body is required for Stripe signature verification.
    const rawBody = await request.text();
    const signature = request.headers.get('stripe-signature');

    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });

    let event;
    try {
      // Async variant + WebCrypto provider are required on the Workers runtime.
      event = await stripe.webhooks.constructEventAsync(
        rawBody,
        signature,
        webhookSecret,
        undefined,
        Stripe.createSubtleCryptoProvider(),
      );
    } catch (err) {
      console.error('Webhook signature verification failed:', err.message);
      return json({ error: 'Invalid signature' }, 400, headers);
    }

    // Ignore events we don't handle
    if (!HANDLED_EVENTS.includes(event.type)) {
      return json({ received: true }, 200, headers);
    }

    try {
      let result;
      switch (event.type) {
        case 'checkout.session.completed':
          result = await handleCheckoutCompleted(env, stripe, event.data.object);
          break;
        case 'invoice.paid':
          result = await handleInvoicePaid(env, stripe, event.data.object);
          break;
        case 'customer.subscription.deleted':
          result = await handleSubscriptionDeleted(stripe, event.data.object);
          break;
      }
      return json({ received: true, ...result }, 200, headers);
    } catch (err) {
      console.error(`Webhook ${event.type} failed:`, err.message);
      return json({ error: 'Webhook processing failed' }, 500, headers);
    }
  }

  // -------------------------------------------------------------------------
  // GET: Retrieve license — two paths with VERY different trust properties.
  //
  //   ?session_id=cs_...  VERIFIED. A Stripe checkout session id is a
  //                       high-entropy, unguessable token issued only to whoever
  //                       completed that checkout, and we re-verify it against
  //                       Stripe before using the email inside it. Holding it is
  //                       proof of purchase, so returning the key here is safe.
  //                       UNCHANGED by the 2026-08-14 fix.
  //
  //   ?email=...          UNVERIFIED caller input — anyone can type anyone's
  //                       address. This path previously returned that address's
  //                       licence key in the response body, which meant one
  //                       unauthenticated GET yielded a full Ed25519-signed key
  //                       for any customer whose email was known or guessed. Those
  //                       keys verify OFFLINE against the app's embedded public
  //                       key, so a stolen one works indefinitely with no further
  //                       server contact and nothing to revoke it with. The
  //                       200-vs-404 split was additionally a customer-list oracle.
  //
  //                       It now MAILS the key to the address on file and returns a
  //                       CONSTANT 202 — identical body, and identical work done
  //                       before responding — whether or not the address matched.
  //                       Control of the mailbox is the authentication factor;
  //                       nothing is disclosed to the caller either way.
  // -------------------------------------------------------------------------
  if (request.method === 'GET') {
    const url = new URL(request.url);
    const session_id = url.searchParams.get('session_id');
    const email = url.searchParams.get('email');

    if (session_id) return handleSessionLookup(env, session_id, headers);
    if (email) return handleEmailRecovery(env, email, headers, waitUntil);

    return json({ error: 'Provide session_id or email' }, 400, headers);
  }

  return json({ error: 'Method not allowed' }, 405, headers);
}

// ---------------------------------------------------------------------------
// GET path 1: verified checkout session -> returns the key (unchanged behaviour)
// ---------------------------------------------------------------------------

async function handleSessionLookup(env, session_id, headers) {
  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });

    let customerEmail;
    try {
      const session = await stripe.checkout.sessions.retrieve(session_id);
      customerEmail = session.customer_email || session.customer_details?.email;
      if (!customerEmail) {
        return json({ error: 'No email found in checkout session' }, 404, headers);
      }
    } catch {
      return json({ error: 'Invalid session' }, 400, headers);
    }

    const customers = await stripe.customers.list({ email: customerEmail.toLowerCase(), limit: 1 });
    if (customers.data.length === 0) {
      return json({ error: 'No license found' }, 404, headers);
    }

    const customer = customers.data[0];
    const license = customer.metadata?.streets_license;
    if (!license) {
      return json({ error: 'No STREETS license found' }, 404, headers);
    }

    // Check expiration before returning
    const expiresAt = customer.metadata.streets_expires_at;
    if (expiresAt && new Date(expiresAt) < new Date()) {
      return json({ error: 'License has expired. Please renew your subscription.', expired_at: expiresAt }, 410, headers);
    }

    return json(
      {
        license_key: license,
        tier: customer.metadata.streets_tier,
        issued_at: customer.metadata.streets_issued_at,
        expires_at: expiresAt,
        status: customer.metadata.streets_status || 'active',
      },
      200,
      headers,
    );
  } catch (err) {
    console.error('License retrieval failed:', err.message);
    return json({ error: 'Failed to retrieve license' }, 500, headers);
  }
}

// ---------------------------------------------------------------------------
// GET path 2: recovery by unverified email -> mails the key, discloses nothing
// ---------------------------------------------------------------------------

// FOLLOW-UP (not fixed here): this path is still unauthenticated and unmetered,
// so it can be driven in a loop to repeatedly mail an existing customer their own
// key. It cannot mail anyone who is NOT already a customer (see recovery-email.js),
// which bounds the blast radius to inbox nuisance rather than open-relay abuse.
// A proper fix needs per-IP/per-address counters, and this Pages project declares
// no KV or D1 binding to hold them (site/wrangler.toml). Two options, neither
// requiring code: a Cloudflare WAF rate-limiting rule on /api/streets/activate
// (dashboard-only, recommended), or adding a KV binding and gating here.
//
// The single response every successful email-recovery request gets, no matter
// what the lookup finds. Keeping this a module constant makes it impossible to
// accidentally branch the body on customer existence.
const RECOVERY_ACCEPTED = {
  delivery: 'email',
  message:
    "If that address has a 4DA licence, we've emailed the key to it. Check your inbox and spam folder.",
};

async function handleEmailRecovery(env, email, headers, waitUntil) {
  // Shape-only rejection. Depends purely on the submitted string, so it cannot
  // distinguish "not a customer" from "not an email".
  if (!isPlausibleEmail(email)) {
    return json({ error: 'Provide a valid email address', reason: 'invalid_email' }, 400, headers);
  }

  // Honest failure when outbound mail is not provisioned. We do NOT fall back to
  // returning the key — that fallback IS the vulnerability. Checked before any
  // Stripe call, so this answer is identical for every address.
  if (!isRecoveryEmailConfigured(env) || !env.STRIPE_SECRET_KEY) {
    console.error('Recovery by email requested but RESEND_API_KEY/RESEND_FROM_EMAIL are unset');
    return json(
      {
        error:
          'Email recovery is temporarily unavailable. Contact support@4da.ai from your purchase email and we will send your key.',
        reason: 'recovery_email_unavailable',
      },
      503,
      headers,
    );
  }

  const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });
  const delivery = deliverRecoveryEmail(env, stripe, email)
    // Logged, never returned: the outcome is exactly the fact we must not leak.
    .then((outcome) => console.log('Recovery by email outcome:', outcome))
    .catch((err) => console.error('Recovery by email crashed:', err?.message));

  // Respond BEFORE the lookup runs. Response latency is therefore constant and
  // carries no information about whether the address is a customer — the timing
  // side of the oracle, which a uniform body alone would not have closed.
  if (typeof waitUntil === 'function') {
    waitUntil(delivery);
  } else {
    // Runtimes without waitUntil (not production Pages): correctness over timing.
    await delivery;
  }

  return json(RECOVERY_ACCEPTED, 202, headers);
}
