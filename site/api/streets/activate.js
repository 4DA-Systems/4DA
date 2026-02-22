// Vercel Serverless: STREETS License Activation
// Handles Stripe webhook (POST) and license retrieval (GET with email query)
//
// Environment variables required:
//   STRIPE_WEBHOOK_SECRET   — Stripe webhook signing secret
//   LICENSE_PRIVATE_KEY_HEX — Ed25519 private key (hex, 64 chars) for signing license keys
//
// License format: 4DA-{base64(json_payload)}.{base64(ed25519_signature)}

import crypto from 'crypto';

// ---------------------------------------------------------------------------
// Ed25519 license generation
// ---------------------------------------------------------------------------

function generateLicenseKey(payload) {
  const privateKeyHex = process.env.LICENSE_PRIVATE_KEY_HEX;
  if (!privateKeyHex) throw new Error('LICENSE_PRIVATE_KEY_HEX not configured');

  const payloadJson = JSON.stringify(payload);
  const payloadBytes = Buffer.from(payloadJson, 'utf8');
  const payloadB64 = payloadBytes.toString('base64');

  // Ed25519 sign
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
// Stripe webhook verification
// ---------------------------------------------------------------------------

function verifyStripeSignature(body, signature, secret) {
  const elements = signature.split(',');
  const timestamp = elements.find(e => e.startsWith('t='))?.slice(2);
  const sigHash = elements.find(e => e.startsWith('v1='))?.slice(3);

  if (!timestamp || !sigHash) return false;

  const signedPayload = `${timestamp}.${body}`;
  const expectedSig = crypto
    .createHmac('sha256', secret)
    .update(signedPayload)
    .digest('hex');

  return crypto.timingSafeEqual(
    Buffer.from(sigHash, 'hex'),
    Buffer.from(expectedSig, 'hex'),
  );
}

// ---------------------------------------------------------------------------
// Simple in-memory store (for serverless, use Vercel KV in production)
// ---------------------------------------------------------------------------

// Note: In production, replace with Vercel KV or a database.
// Serverless functions are stateless — this is a placeholder.
const licenseStore = new Map();

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

export default async function handler(req, res) {
  // CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Stripe-Signature');

  if (req.method === 'OPTIONS') {
    return res.status(200).end();
  }

  // POST: Stripe webhook — generate license on successful payment
  if (req.method === 'POST') {
    const stripeSecret = process.env.STRIPE_WEBHOOK_SECRET;
    if (!stripeSecret) {
      return res.status(500).json({ error: 'Webhook secret not configured' });
    }

    const signature = req.headers['stripe-signature'];
    const rawBody = typeof req.body === 'string' ? req.body : JSON.stringify(req.body);

    if (!signature || !verifyStripeSignature(rawBody, signature, stripeSecret)) {
      return res.status(400).json({ error: 'Invalid signature' });
    }

    const event = typeof req.body === 'string' ? JSON.parse(req.body) : req.body;

    if (event.type !== 'checkout.session.completed') {
      return res.status(200).json({ received: true });
    }

    const session = event.data.object;
    const email = session.customer_email || session.customer_details?.email;
    const tier = session.metadata?.streets_tier || 'community';

    if (!email) {
      return res.status(400).json({ error: 'No customer email' });
    }

    // Determine features based on tier
    const features = tier === 'cohort'
      ? ['streets_community', 'streets_cohort']
      : ['streets_community'];

    // Generate license
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
      // Store for retrieval
      licenseStore.set(email.toLowerCase(), {
        key: licenseKey,
        tier,
        created_at: now.toISOString(),
      });
      return res.status(200).json({ received: true, license_generated: true });
    } catch (err) {
      console.error('License generation failed:', err);
      return res.status(500).json({ error: 'License generation failed' });
    }
  }

  // GET: Retrieve license by email
  if (req.method === 'GET') {
    const { email } = req.query;

    if (!email) {
      return res.status(400).json({ error: 'Email required' });
    }

    const stored = licenseStore.get(email.toLowerCase());
    if (!stored) {
      return res.status(404).json({ error: 'No license found for this email' });
    }

    return res.status(200).json({
      license_key: stored.key,
      tier: stored.tier,
      created_at: stored.created_at,
    });
  }

  return res.status(405).json({ error: 'Method not allowed' });
}
