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
//   - charge.refunded              -> refund (mark metadata terminal)
//   - charge.dispute.created       -> chargeback (mark metadata terminal)
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
  hasOtherStandingCharge,
  isRevoked,
  isTerminal,
  meta,
  metaKey,
  resolveCustomerId,
  sessionProvesPurchase,
  sessionWithinWindow,
  stripeIdOf,
  terminalStatusPatch,
} from '../../../lib/entitlement.js';
import {
  checkAndCount,
  ipWindowKey,
  IP_WINDOW_TTL_SECONDS,
  isDuplicateEvent,
  mailWindowKey,
  MAIL_WINDOW_TTL_SECONDS,
  markEventProcessed,
  RECOVERY_MAILS_PER_ADDRESS_PER_DAY,
  RECOVERY_REQUESTS_PER_IP_PER_HOUR,
} from '../../../lib/abuse-guards.js';
import {
  deliverLicenseEmail,
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
      [metaKey('license')]: licenseKey,
      [metaKey('tier')]: effectiveTier,
      [metaKey('billing_period')]: period,
      [metaKey('issued_at')]: now.toISOString(),
      [metaKey('expires_at')]: expiresAt.toISOString(),
      [metaKey('status')]: 'active',
    },
  });

  return { licenseKey, expiresAt };
}

// resolveCustomerId now lives in ../../../lib/entitlement.js — dependency-free
// so it can be regression-tested (site/ has no installed dependency tree in CI).
// The move fixed an ordering bug: it lower-cased `email` BEFORE the caller's
// `if (!email)` guard ran, so a session with no email threw a TypeError and the
// webhook answered a bare 500 that Stripe then retried.

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

const HANDLED_EVENTS = [
  'checkout.session.completed',
  'invoice.paid',
  'customer.subscription.deleted',
  // Money going BACK to the customer used to be invisible to this webhook. See
  // the honesty note above handleChargeRefunded for exactly what these do and,
  // more importantly, what they do not do.
  'charge.refunded',
  'charge.dispute.created',
  // A won dispute reverses the chargeback lockout — see handleDisputeClosed.
  'charge.dispute.closed',
];

async function handleCheckoutCompleted(env, stripe, session) {
  const email = session.customer_email || session.customer_details?.email;
  const tier = meta(session.metadata, 'tier') || 'signal';
  const billingPeriod = session.metadata?.billing_period;

  // ORDERING: this guard must run BEFORE resolveCustomerId, which normalises
  // the address with `.toLowerCase()`. The two used to be the other way round,
  // so a session carrying neither a customer id nor an email produced
  // `TypeError: Cannot read properties of null (reading 'toLowerCase')` and a
  // generic 500 that named nothing, instead of this diagnosable error. The
  // licence payload embeds the email, so there is no path that needs it later.
  if (!email) {
    throw new Error(`No customer email in session ${session.id}`);
  }

  const customerId = await resolveCustomerId(stripe, session.customer, email);

  const { licenseKey, expiresAt } = await generateAndStoreLicense(env, stripe, customerId, email, tier, billingPeriod);
  // Lease model: back the account with a stable refresh credential for the
  // short-lived-token flow (additive; legacy long token above still delivered).
  const refreshKey = await ensureRefreshKey(stripe, customerId);

  // Mail the buyer their own copy. Until this existed, the success page
  // rendering the key was the ONLY delivery: a buyer who closed the tab inside
  // the webhook window (the page retries 4x over ~8s) had nothing, and the
  // recovery fallback was unprovisioned and answered 503. Awaited rather than
  // backgrounded because it cannot throw — see deliverLicenseEmail — so the log
  // line is guaranteed written before we return 200 to Stripe.
  const mailed = await deliverLicenseEmail(
    env,
    email,
    licenseKey,
    tier,
    expiresAt?.toISOString?.() ?? expiresAt,
    'purchase',
  );

  console.log('License generated:', email, 'tier:', tier, 'period:', billingPeriod, 'customer:', customerId, 'len:', licenseKey.length, 'refresh_key:', refreshKey ? 'set' : 'none', 'emailed:', mailed);
  return { license_generated: true, emailed: mailed };
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
  const existingTier = meta(customer.metadata, 'tier') || 'signal';
  // Preserve the billing period across renewals so annual keys stay annual.
  const billingPeriod = meta(customer.metadata, 'billing_period');

  // A renewal must not resurrect a terminated customer. `generateAndStoreLicense`
  // writes `streets_status: 'active'` unconditionally, so without this guard the
  // sequence [invoice.paid -> charge.refunded -> invoice.paid re-delivered] ends
  // with the customer active again holding a brand-new key on a fresh expiry —
  // which is the exact defect this file's refund handling exists to close, and
  // Stripe re-delivering an already-processed event is the ordinary case, not an
  // edge one (there is no event-id dedup store; see the dispatch note below).
  //
  // Recovery is deliberately narrow: only a fresh `checkout.session.completed`
  // clears a terminal status, because that is someone actually paying again.
  if (isTerminal(customer.metadata)) {
    console.log('Renewal ignored for terminated customer:', customerId, 'status:', meta(customer.metadata, 'status'));
    return { skipped: `customer is ${meta(customer.metadata, 'status')} — renewal does not re-issue` };
  }

  if (!email) {
    throw new Error(`No email for customer ${customerId}`);
  }

  // Regenerate license with fresh expiry
  const { licenseKey, expiresAt } = await generateAndStoreLicense(env, stripe, customerId, email, existingTier, billingPeriod);

  // A renewal SILENTLY replaces the key the customer is holding — the previous
  // one stops working at its original expiry. Without this mail the first they
  // learn of it is the app refusing their key, so a renewal has to be delivered
  // as deliberately as a purchase.
  const mailed = await deliverLicenseEmail(
    env,
    email,
    licenseKey,
    existingTier,
    expiresAt?.toISOString?.() ?? expiresAt,
    'renewal',
  );

  console.log('License renewed:', email, 'tier:', existingTier, 'period:', billingPeriod, 'customer:', customerId, 'reason:', invoice.billing_reason, 'len:', licenseKey.length, 'emailed:', mailed);
  return { license_renewed: true, emailed: mailed };
}

// ---------------------------------------------------------------------------
// Terminal entitlement transitions: cancellation, refund, chargeback.
//
// READ THIS BEFORE BELIEVING THE WORD "REVOKE" ANYWHERE NEAR THIS CODE.
//
// These handlers do NOT revoke an already-issued licence key, and nothing here
// can. Keys are Ed25519-signed and verified OFFLINE by the desktop app against
// a public key embedded in the binary (src-tauri/src/settings/license/
// verify.rs). Once a key is on a user's machine it keeps working, with zero
// server contact, until the `expires_at` INSIDE the signed payload passes:
//
//     monthly   ~35 days      annual   ~1 year + 7 days      lifetime   2099
//
// What writing a terminal status here DOES achieve:
//   * invoice.paid stops re-issuing a fresh key with a fresh expiry — enforced
//     by the isTerminal() guard in handleInvoicePaid, NOT by the status write
//     on its own: generateAndStoreLicense sets `streets_status: 'active'`
//     unconditionally, so without that guard a re-delivered renewal silently
//     un-terminates the customer;
//   * /api/license/refresh stops minting new lease tokens for the lifetime tier
//     (functions/api/license/refresh.js, via isLifetimeEntitled());
//   * the session_id GET path reports the status to the buyer.
//
// So a refunded MONTHLY customer loses access within ~35 days, and a refunded
// LIFETIME customer keeps the key they are already holding until 2099. That is
// the honest statement of the current position. Before this change there was no
// refund or chargeback handler at all, so a refunded customer was never even
// marked, and `refresh.js` gated on a 'refunded' status that no writer produced.
//
// RECOMMENDATION, deliberately NOT implemented here because it is a licence
// FORMAT change and needs its own review: issue lifetime purchases a
// short-dated key like every other tier and let the refresh endpoint renew it.
// That makes refresh.js load-bearing for all tiers and turns this metadata
// write into real revocation with a bounded window, instead of a 73-year one.
// ---------------------------------------------------------------------------

async function applyTerminalStatus(stripe, customerId, status, context) {
  const customer = await stripe.customers.retrieve(customerId);
  if (customer?.deleted) {
    return { skipped: 'customer deleted' };
  }

  // terminalStatusPatch is idempotent by construction: it preserves the
  // first-seen timestamp and never downgrades severity, so a duplicate Stripe
  // delivery writes exactly the same bytes. Stripe MERGES customer metadata on
  // update, so naming only the changed keys is both the smallest write and the
  // one that cannot clobber the licence stored alongside it.
  const patch = terminalStatusPatch(customer.metadata, status, new Date().toISOString());
  await stripe.customers.update(customerId, { metadata: patch });

  const effective = patch[metaKey('status')] || meta(customer.metadata, 'status') || status;
  console.log(
    'Entitlement terminated:',
    status,
    'effective:',
    effective,
    'customer:',
    customerId,
    'context:',
    JSON.stringify(context),
  );
  return { entitlement_status: effective };
}

async function handleSubscriptionDeleted(stripe, subscription) {
  const customerId = stripeIdOf(subscription.customer);
  if (!customerId) {
    return { skipped: 'no customer ID' };
  }
  return applyTerminalStatus(stripe, customerId, 'cancelled', { subscription: subscription.id });
}

async function handleChargeRefunded(stripe, charge) {
  // Stripe fires charge.refunded for PARTIAL refunds too, and sets
  // `charge.refunded === true` only once the charge is fully refunded. A $5
  // goodwill credit on an annual plan is not a cancelled purchase, so a partial
  // refund must leave entitlement exactly where it is.
  if (charge.refunded !== true) {
    return { skipped: 'partial refund — entitlement unchanged' };
  }

  const customerId = stripeIdOf(charge.customer);
  if (!customerId) {
    return { skipped: 'no customer on charge' };
  }

  // Terminal status lands on the CUSTOMER; the refund happened to a CHARGE.
  // If any other payment of theirs is still standing, this refund did not end
  // their entitlement — see hasOtherStandingCharge for the cases this protects.
  if (await hasOtherStandingCharge(stripe, customerId, charge.id)) {
    return { skipped: 'another paid charge still stands — entitlement unchanged' };
  }

  return applyTerminalStatus(stripe, customerId, 'refunded', { charge: charge.id });
}

// The Dispute object carries no `customer` field, and `dispute.charge` is an id
// string on most API versions and an expanded object on some. Resolve through
// whichever we were handed rather than assuming. Shared by created + closed.
async function resolveDisputeCustomer(stripe, dispute) {
  if (dispute.charge && typeof dispute.charge === 'object') {
    const id = stripeIdOf(dispute.charge.customer);
    if (id) return id;
  }
  const chargeId = stripeIdOf(dispute.charge);
  if (chargeId) {
    const charge = await stripe.charges.retrieve(chargeId);
    return stripeIdOf(charge?.customer);
  }
  return null;
}

async function handleDisputeCreated(stripe, dispute) {
  const customerId = await resolveDisputeCustomer(stripe, dispute);
  if (!customerId) {
    return { skipped: 'no customer resolvable from dispute' };
  }

  // Same purchase-vs-customer distinction as a refund. A dispute on one charge
  // does not end an entitlement another, undisputed payment still covers.
  if (await hasOtherStandingCharge(stripe, customerId, stripeIdOf(dispute.charge))) {
    return { skipped: 'another paid charge still stands — entitlement unchanged' };
  }

  return applyTerminalStatus(stripe, customerId, 'chargeback', { dispute: dispute.id });
}

// A dispute we WON (status 'won') means the bank rejected the customer's
// chargeback — they did pay and keep the money with us — so the chargeback that
// locked them out was reversed and their access must come back. Without this a
// monthly subscriber whose dispute we won stays terminal forever: the
// isTerminal guard in handleInvoicePaid then blocks every future renewal from
// re-issuing, and only a fresh checkout.session.completed clears terminal — which
// a renewing subscriber never generates. Deliberately narrow: only 'won', and
// only when the customer is CURRENTLY in the exact 'chargeback' state (a later
// refund is more severe and must stand; a cancellation is a different episode).
async function handleDisputeClosed(stripe, dispute) {
  if (dispute.status !== 'won') {
    return { skipped: `dispute ${dispute.status || 'closed'} — entitlement unchanged` };
  }
  const customerId = await resolveDisputeCustomer(stripe, dispute);
  if (!customerId) {
    return { skipped: 'no customer resolvable from dispute' };
  }
  const customer = await stripe.customers.retrieve(customerId);
  if (!customer || customer.deleted) {
    return { skipped: 'customer deleted' };
  }
  if (meta(customer.metadata, 'status') !== 'chargeback') {
    return { skipped: 'not in chargeback state — nothing to restore' };
  }
  await stripe.customers.update(customerId, { metadata: { [metaKey('status')]: 'active' } });
  console.log('Entitlement restored after won dispute:', customerId, 'dispute:', dispute.id);
  return { entitlement_restored: true };
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

    // -----------------------------------------------------------------------
    // DUPLICATE DELIVERIES — layered defence.
    //
    // Stripe retries a webhook until it gets a 2xx and routinely re-sends the
    // same event id. Three layers bound what a duplicate can do:
    //   * constructEventAsync above enforces Stripe's 300s signature replay
    //     tolerance, so a captured request body cannot be replayed by a third
    //     party indefinitely. Only Stripe's own retries reach these handlers
    //     twice, and only on its retry schedule.
    //   * The three TERMINAL handlers (customer.subscription.deleted,
    //     charge.refunded, charge.dispute.created) are idempotent BY
    //     CONSTRUCTION: terminalStatusPatch() preserves the first-seen
    //     timestamp and never downgrades severity, so re-delivery — in any
    //     order — writes identical bytes.
    //   * The MINTING handlers (checkout.session.completed, invoice.paid)
    //     issue a new signed key on every run and cannot be idempotent by
    //     construction — so the LICENSE_KV event-id gate below skips any event
    //     id that already completed successfully. The id is recorded only
    //     AFTER the handler succeeds: a failed handler stays retryable.
    //
    // FAIL-OPEN: a missing binding or a KV error processes the event anyway
    // (worst case is yesterday's status quo — a duplicate key), because the
    // alternative is failing the live payment path on a lookup store.
    // -----------------------------------------------------------------------

    const dedupStore = env.LICENSE_KV;
    if (!dedupStore) {
      console.error('LICENSE_KV binding missing — webhook dedup disabled for this delivery');
    } else if (await isDuplicateEvent(dedupStore, event.id)) {
      console.log('Duplicate webhook delivery skipped:', event.type, event.id);
      return json({ received: true, duplicate: true }, 200, headers);
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
        case 'charge.refunded':
          result = await handleChargeRefunded(stripe, event.data.object);
          break;
        case 'charge.dispute.created':
          result = await handleDisputeCreated(stripe, event.data.object);
          break;
        case 'charge.dispute.closed':
          result = await handleDisputeClosed(stripe, event.data.object);
          break;
      }
      if (dedupStore) await markEventProcessed(dedupStore, event.id);
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
    if (email) return handleEmailRecovery(env, request, email, headers, waitUntil);

    return json({ error: 'Provide session_id or email' }, 400, headers);
  }

  return json({ error: 'Method not allowed' }, 405, headers);
}

// ---------------------------------------------------------------------------
// GET path 1: verified checkout session -> returns the key (unchanged behaviour)
// ---------------------------------------------------------------------------

// A completed checkout session is old enough to have been fulfilled and emailed;
// beyond this window the session id (which sits in the success-page URL and is a
// bearer credential for the key) stops being honoured, so a URL that later
// surfaces in browser history or a shared link cannot retrieve the key. Recovery
// by email covers anyone who needs the key after the window.
const SESSION_LOOKUP_MAX_AGE_SECONDS = 24 * 60 * 60;

async function handleSessionLookup(env, session_id, headers) {
  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });

    let session;
    try {
      session = await stripe.checkout.sessions.retrieve(session_id);
    } catch {
      return json({ error: 'Invalid session' }, 400, headers);
    }

    // GATE 1 — the session must PROVE a completed purchase (see
    // sessionProvesPurchase). An incomplete session id + a buyer-typed email was
    // a full-key oracle for any customer whose email an attacker knew.
    if (!sessionProvesPurchase(session)) {
      return json({ error: 'Checkout not completed' }, 402, headers);
    }

    // GATE 2 — bind to the session's OWN customer, never a list-by-email. The
    // webhook wrote the licence onto exactly this customer (resolveCustomerId
    // returns session.customer unchanged when present); an email lookup could
    // return a different, newer customer record for the same address
    // (customer_creation:'always' on lifetime makes duplicates) — the read/write
    // mismatch that made this an email oracle in the first place.
    const customerId = stripeIdOf(session.customer);
    if (!customerId) {
      return json({ error: 'No license found' }, 404, headers);
    }

    // GATE 3 — time-box the bearer window (see sessionWithinWindow).
    if (!sessionWithinWindow(session, SESSION_LOOKUP_MAX_AGE_SECONDS, Math.floor(Date.now() / 1000))) {
      return json(
        { error: 'This checkout link has expired. Use email recovery to get your key.', reason: 'session_expired' },
        410,
        headers,
      );
    }

    const customer = await stripe.customers.retrieve(customerId);
    if (!customer || customer.deleted) {
      return json({ error: 'No license found' }, 404, headers);
    }

    const license = meta(customer.metadata, 'license');
    if (!license) {
      return json({ error: 'No STREETS license found' }, 404, headers);
    }

    // Refunded / charged back: holding the checkout session id proves this
    // purchase happened, but the money has since gone back, so the key is not
    // re-issued. A merely CANCELLED subscriber is NOT blocked here — their paid
    // tail runs to the key's own expiry, which is the policy being sold.
    if (isRevoked(customer.metadata)) {
      return json(
        {
          error: 'This licence is no longer active.',
          status: meta(customer.metadata, 'status'),
        },
        410,
        headers,
      );
    }

    // Check expiration before returning
    const expiresAt = meta(customer.metadata, 'expires_at');
    if (expiresAt && new Date(expiresAt) < new Date()) {
      return json({ error: 'License has expired. Please renew your subscription.', expired_at: expiresAt }, 410, headers);
    }

    return json(
      {
        license_key: license,
        tier: meta(customer.metadata, 'tier'),
        issued_at: meta(customer.metadata, 'issued_at'),
        expires_at: expiresAt,
        status: meta(customer.metadata, 'status') || 'active',
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

// This path is unauthenticated by design (control of the mailbox is the auth
// factor), so it is METERED instead — LICENSE_KV counters, two windows:
//
//   * per IP / hour: bounds scripted probing before any work happens. Answered
//     with an honest 429 — the limit depends only on the CALLER's behaviour,
//     never on whether any address is a customer, so it leaks nothing.
//   * per address / day: bounds how many mails one inbox can be sent, and —
//     the sharper risk — how much of the Resend daily quota an attacker who
//     knows ONE customer address can burn (quota exhaustion would silence the
//     licence-delivery mail of every real purchase that day). Enforced INSIDE
//     the deferred delivery, after the constant 202 is already sent, so the
//     response body and timing stay identical in every case.
//
// Both fail OPEN on KV errors: recovery for a real customer is never blocked
// by a broken counter store. A WAF rate-limiting rule on /api/streets/activate
// remains worthwhile belt-and-braces (dashboard-only, zone-level).
//
// The single response every successful email-recovery request gets, no matter
// what the lookup finds. Keeping this a module constant makes it impossible to
// accidentally branch the body on customer existence.
const RECOVERY_ACCEPTED = {
  delivery: 'email',
  message:
    "If that address has a 4DA licence, we've emailed the key to it. Check your inbox and spam folder.",
};

async function handleEmailRecovery(env, request, email, headers, waitUntil) {
  // Shape-only rejection. Depends purely on the submitted string, so it cannot
  // distinguish "not a customer" from "not an email".
  if (!isPlausibleEmail(email)) {
    return json({ error: 'Provide a valid email address', reason: 'invalid_email' }, 400, headers);
  }

  // Per-IP window. Runs the same for every request from this caller regardless
  // of the address submitted, so the 429 carries no customer information.
  const kv = env.LICENSE_KV;
  const callerIp = request.headers.get('cf-connecting-ip');
  if (kv && callerIp) {
    const allowed = await checkAndCount(
      kv,
      ipWindowKey(callerIp),
      RECOVERY_REQUESTS_PER_IP_PER_HOUR,
      IP_WINDOW_TTL_SECONDS,
    );
    if (!allowed) {
      return json(
        { error: 'Too many recovery requests. Please try again later.', reason: 'rate_limited' },
        429,
        headers,
      );
    }
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
  const delivery = (async () => {
    // Per-address window, checked AFTER the constant 202 has gone out (this
    // whole closure rides waitUntil), so hitting the cap changes nothing the
    // caller can observe — the mail is simply not sent again.
    if (kv) {
      const allowed = await checkAndCount(
        kv,
        mailWindowKey(email),
        RECOVERY_MAILS_PER_ADDRESS_PER_DAY,
        MAIL_WINDOW_TTL_SECONDS,
      );
      if (!allowed) {
        console.log('Recovery by email outcome: rate_limited (per-address daily cap)');
        return;
      }
    }
    // Logged, never returned: the outcome is exactly the fact we must not leak.
    const outcome = await deliverRecoveryEmail(env, stripe, email);
    console.log('Recovery by email outcome:', outcome);
  })().catch((err) => console.error('Recovery by email crashed:', err?.message));

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
