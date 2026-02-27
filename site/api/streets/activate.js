// Vercel Serverless: STREETS License Activation
//
// POST: Stripe webhook — handles:
//   - checkout.session.completed  → initial license generation
//   - invoice.paid                → subscription renewal (fresh license + extended expiry)
//   - customer.subscription.deleted → cancellation (mark metadata)
//   - checkout.session.completed with upgrade → cohort upgrade from community
// GET:  Retrieve license by checkout session_id (secure) or email (fallback)
//
// Environment variables required:
//   STRIPE_SECRET_KEY       — Stripe secret key
//   STRIPE_WEBHOOK_SECRET   — Stripe webhook signing secret
//   LICENSE_PRIVATE_KEY_HEX — Ed25519 private key (hex, 64 chars) for signing license keys

import crypto from 'crypto';
import Stripe from 'stripe';

// ---------------------------------------------------------------------------
// Ed25519 license generation
// Both Node.js crypto and Rust ed25519_dalek implement RFC 8032 "pure" Ed25519.
// The signature format (64 bytes, standard base64) is cross-platform compatible.
// ---------------------------------------------------------------------------

function generateLicenseKey(payload) {
  const privateKeyHex = process.env.LICENSE_PRIVATE_KEY_HEX;
  if (!privateKeyHex) throw new Error('LICENSE_PRIVATE_KEY_HEX not configured');

  const payloadJson = JSON.stringify(payload);
  const payloadBytes = Buffer.from(payloadJson, 'utf8');
  const payloadB64 = payloadBytes.toString('base64');

  const privateKeyBuffer = Buffer.from(privateKeyHex, 'hex');
  const keyObject = crypto.createPrivateKey({
    key: Buffer.concat([
      // DER prefix for Ed25519 private key (RFC 8410)
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      privateKeyBuffer,
    ]),
    format: 'der',
    type: 'pkcs8',
  });

  const signature = crypto.sign(null, payloadBytes, keyObject);
  const sigB64 = signature.toString('base64');

  return `4DA-${payloadB64}.${sigB64}`;
}

// ---------------------------------------------------------------------------
// Stripe client (lazy init — persists across warm invocations)
// ---------------------------------------------------------------------------

let _stripe;
function getStripe() {
  if (!_stripe) {
    const key = process.env.STRIPE_SECRET_KEY;
    if (!key) throw new Error('STRIPE_SECRET_KEY not configured');
    _stripe = new Stripe(key);
  }
  return _stripe;
}

// ---------------------------------------------------------------------------
// CORS — scope to known origins
// ---------------------------------------------------------------------------

// localhost only in non-production; tauri:// always allowed (desktop app)
const ALLOWED_ORIGINS = [
  'https://4da.ai',
  'https://www.4da.ai',
  'https://streets.4da.ai',
  'tauri://localhost',
  ...(process.env.VERCEL_ENV !== 'production'
    ? ['http://localhost:4444', 'http://localhost:1420']
    : []),
];

function setCors(req, res) {
  const origin = req.headers.origin;
  if (origin && ALLOWED_ORIGINS.includes(origin)) {
    res.setHeader('Access-Control-Allow-Origin', origin);
    res.setHeader('Vary', 'Origin');
  }
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Stripe-Signature');
}

// ---------------------------------------------------------------------------
// Shared: generate + store license for a customer
// ---------------------------------------------------------------------------

async function generateAndStoreLicense(stripe, customerId, email, tier) {
  const features = tier === 'cohort'
    ? ['streets_community', 'streets_cohort']
    : ['streets_community'];

  const now = new Date();
  const expiresAt = new Date(now);
  expiresAt.setFullYear(expiresAt.getFullYear() + 1);

  const payload = {
    tier,
    email,
    expires_at: expiresAt.toISOString(),
    issued_at: now.toISOString(),
    features,
  };

  const licenseKey = generateLicenseKey(payload);

  // Guard: Stripe metadata values max 500 chars
  if (licenseKey.length > 500) {
    throw new Error(`License key exceeds Stripe metadata limit: ${licenseKey.length} chars`);
  }

  await stripe.customers.update(customerId, {
    metadata: {
      streets_license: licenseKey,
      streets_tier: tier,
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

// ---------------------------------------------------------------------------
// Webhook event handlers
// ---------------------------------------------------------------------------

const HANDLED_EVENTS = [
  'checkout.session.completed',
  'invoice.paid',
  'customer.subscription.deleted',
];

async function handleCheckoutCompleted(stripe, session) {
  const email = session.customer_email || session.customer_details?.email;
  const customerId = await resolveCustomerId(stripe, session.customer, email);
  const tier = session.metadata?.streets_tier || 'community';

  if (!email) {
    throw new Error(`No customer email in session ${session.id}`);
  }

  // Check if this is an upgrade: existing customer with community → buying cohort
  const customer = await stripe.customers.retrieve(customerId);
  const existingTier = customer.metadata?.streets_tier;
  const effectiveTier = (existingTier === 'community' && tier === 'cohort') ? 'cohort' : tier;

  const { licenseKey } = await generateAndStoreLicense(stripe, customerId, email, effectiveTier);
  console.log('License generated:', email, 'tier:', effectiveTier, 'customer:', customerId, 'len:', licenseKey.length);
  return { license_generated: true };
}

async function handleInvoicePaid(stripe, invoice) {
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
  const existingTier = customer.metadata?.streets_tier || 'community';

  if (!email) {
    throw new Error(`No email for customer ${customerId}`);
  }

  // Regenerate license with fresh expiry
  const { licenseKey } = await generateAndStoreLicense(stripe, customerId, email, existingTier);
  console.log('License renewed:', email, 'tier:', existingTier, 'customer:', customerId, 'reason:', invoice.billing_reason);
  return { license_renewed: true };
}

async function handleSubscriptionDeleted(stripe, subscription) {
  const customerId = subscription.customer;
  if (!customerId) {
    return { skipped: 'no customer ID' };
  }

  const customer = await stripe.customers.retrieve(customerId);

  // Don't revoke immediately — the existing license key is still valid until
  // its embedded expires_at date. Just mark the status so the app can show
  // a "subscription cancelled" message and the GET endpoint can inform the user.
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

export default async function handler(req, res) {
  setCors(req, res);

  if (req.method === 'OPTIONS') return res.status(200).end();

  // -------------------------------------------------------------------------
  // POST: Stripe webhook
  // -------------------------------------------------------------------------
  if (req.method === 'POST') {
    const webhookSecret = process.env.STRIPE_WEBHOOK_SECRET;
    if (!webhookSecret) {
      return res.status(500).json({ error: 'Webhook secret not configured' });
    }

    // With bodyParser disabled, read the raw body from the stream
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const rawBody = Buffer.concat(chunks).toString('utf8');

    const stripe = getStripe();
    const signature = req.headers['stripe-signature'];

    let event;
    try {
      event = stripe.webhooks.constructEvent(rawBody, signature, webhookSecret);
    } catch (err) {
      console.error('Webhook signature verification failed:', err.message);
      return res.status(400).json({ error: 'Invalid signature' });
    }

    // Ignore events we don't handle
    if (!HANDLED_EVENTS.includes(event.type)) {
      return res.status(200).json({ received: true });
    }

    try {
      let result;
      switch (event.type) {
        case 'checkout.session.completed':
          result = await handleCheckoutCompleted(stripe, event.data.object);
          break;
        case 'invoice.paid':
          result = await handleInvoicePaid(stripe, event.data.object);
          break;
        case 'customer.subscription.deleted':
          result = await handleSubscriptionDeleted(stripe, event.data.object);
          break;
      }
      return res.status(200).json({ received: true, ...result });
    } catch (err) {
      console.error(`Webhook ${event.type} failed:`, err.message);
      return res.status(500).json({ error: 'Webhook processing failed' });
    }
  }

  // -------------------------------------------------------------------------
  // GET: Retrieve license
  //   - ?session_id=cs_... (secure: verifies checkout session ownership)
  //   - ?email=user@... (fallback: for returning users who lost their key)
  // -------------------------------------------------------------------------
  if (req.method === 'GET') {
    const { session_id, email } = req.query;

    if (!session_id && !email) {
      return res.status(400).json({ error: 'Provide session_id or email' });
    }

    try {
      const stripe = getStripe();
      let customerEmail;

      // Preferred path: verify checkout session and extract email from it
      if (session_id) {
        try {
          const session = await stripe.checkout.sessions.retrieve(session_id);
          customerEmail = session.customer_email || session.customer_details?.email;
          if (!customerEmail) {
            return res.status(404).json({ error: 'No email found in checkout session' });
          }
        } catch {
          return res.status(400).json({ error: 'Invalid session' });
        }
      } else {
        customerEmail = email;
      }

      const customers = await stripe.customers.list({
        email: customerEmail.toLowerCase(),
        limit: 1,
      });

      if (customers.data.length === 0) {
        return res.status(404).json({ error: 'No license found' });
      }

      const customer = customers.data[0];
      const license = customer.metadata?.streets_license;

      if (!license) {
        return res.status(404).json({ error: 'No STREETS license found' });
      }

      // Check expiration before returning
      const expiresAt = customer.metadata.streets_expires_at;
      if (expiresAt && new Date(expiresAt) < new Date()) {
        return res.status(410).json({
          error: 'License has expired. Please renew your subscription.',
          expired_at: expiresAt,
        });
      }

      return res.status(200).json({
        license_key: license,
        tier: customer.metadata.streets_tier,
        issued_at: customer.metadata.streets_issued_at,
        expires_at: expiresAt,
        status: customer.metadata.streets_status || 'active',
      });
    } catch (err) {
      console.error('License retrieval failed:', err.message);
      return res.status(500).json({ error: 'Failed to retrieve license' });
    }
  }

  return res.status(405).json({ error: 'Method not allowed' });
}

// Disable Vercel's automatic body parsing so Stripe webhook signature
// verification works correctly with the raw request body.
export const config = {
  api: { bodyParser: false },
};
