// Vercel Serverless: STREETS License Activation
//
// POST: Stripe webhook — generates Ed25519-signed license, stores in Stripe customer metadata
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

const ALLOWED_ORIGINS = [
  'https://4da.ai',
  'https://www.4da.ai',
  'https://streets.4da.ai',
  'http://localhost:4444',
  'http://localhost:1420',
  'tauri://localhost',
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
// Handler
// ---------------------------------------------------------------------------

export default async function handler(req, res) {
  setCors(req, res);

  if (req.method === 'OPTIONS') return res.status(200).end();

  // -------------------------------------------------------------------------
  // POST: Stripe webhook — generate license on successful checkout
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

    if (event.type !== 'checkout.session.completed') {
      return res.status(200).json({ received: true });
    }

    const session = event.data.object;
    const email = session.customer_email || session.customer_details?.email;
    const customerId = session.customer;
    const tier = session.metadata?.streets_tier || 'community';

    if (!email) {
      console.error('Webhook: no customer email in session', session.id);
      return res.status(400).json({ error: 'No customer email' });
    }

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

    try {
      const licenseKey = generateLicenseKey(payload);

      // Guard: Stripe metadata values max 500 chars
      if (licenseKey.length > 500) {
        console.error('License key exceeds Stripe metadata limit:', licenseKey.length, 'chars, email:', email);
        return res.status(500).json({ error: 'License generation failed' });
      }

      const licenseMetadata = {
        streets_license: licenseKey,
        streets_tier: tier,
        streets_issued_at: now.toISOString(),
        streets_expires_at: expiresAt.toISOString(),
      };

      // Store license in Stripe customer metadata for retrieval.
      // If customerId is missing (shouldn't happen with our checkout config),
      // fall back to searching by email and creating a customer if needed.
      let targetCustomerId = customerId;
      if (!targetCustomerId) {
        const existing = await stripe.customers.list({ email: email.toLowerCase(), limit: 1 });
        if (existing.data.length > 0) {
          targetCustomerId = existing.data[0].id;
        } else {
          const created = await stripe.customers.create({ email: email.toLowerCase() });
          targetCustomerId = created.id;
        }
      }

      await stripe.customers.update(targetCustomerId, { metadata: licenseMetadata });

      console.log('License generated for', email, 'tier:', tier, 'customer:', targetCustomerId);
      return res.status(200).json({ received: true, license_generated: true });
    } catch (err) {
      console.error('License generation failed:', err.message);
      return res.status(500).json({ error: 'License generation failed' });
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
