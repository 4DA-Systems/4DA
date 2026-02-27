// Vercel Serverless: STREETS Checkout Session Creator
//
// Creates Stripe Checkout sessions for STREETS tiers.
// Environment variables required:
//   STRIPE_SECRET_KEY       — Stripe secret key (sk_live_... or sk_test_...)
//   STREETS_PRICE_COMMUNITY — Stripe price ID for Community monthly ($29/mo)
//   STREETS_PRICE_ANNUAL    — Stripe price ID for Community annual ($249/yr)
//   STREETS_PRICE_COHORT    — Stripe price ID for Cohort one-time ($499)
//   SITE_URL                — Base URL for redirects (e.g. https://4da.ai)

import Stripe from 'stripe';

const TIERS = {
  community: {
    priceEnv: 'STREETS_PRICE_COMMUNITY',
    mode: 'subscription',
    metadata: { streets_tier: 'community' },
  },
  annual: {
    priceEnv: 'STREETS_PRICE_ANNUAL',
    mode: 'subscription',
    metadata: { streets_tier: 'community' }, // Same tier as monthly, different billing period
  },
  cohort: {
    priceEnv: 'STREETS_PRICE_COHORT',
    mode: 'payment',
    metadata: { streets_tier: 'cohort' },
  },
};

// Lazy-init Stripe client (survives across warm invocations)
let _stripe;
function getStripe() {
  if (!_stripe) {
    const key = process.env.STRIPE_SECRET_KEY;
    if (!key) throw new Error('STRIPE_SECRET_KEY not configured');
    _stripe = new Stripe(key);
  }
  return _stripe;
}

// CORS — scope to known origins (localhost only in non-production)
const ALLOWED_ORIGINS = [
  'https://4da.ai',
  'https://www.4da.ai',
  'https://streets.4da.ai',
  ...(process.env.VERCEL_ENV !== 'production'
    ? ['http://localhost:4444', 'http://localhost:1420']
    : []),
];

export default async function handler(req, res) {
  const origin = req.headers.origin;
  if (origin && ALLOWED_ORIGINS.includes(origin)) {
    res.setHeader('Access-Control-Allow-Origin', origin);
    res.setHeader('Vary', 'Origin');
  }
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') return res.status(200).end();
  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  // Parse body safely — Vercel auto-parses JSON when bodyParser is enabled
  let body;
  try {
    body = typeof req.body === 'string' ? JSON.parse(req.body) : (req.body || {});
  } catch {
    return res.status(400).json({ error: 'Invalid request body' });
  }

  const { tier } = body;
  const config = TIERS[tier];
  if (!config) {
    return res.status(400).json({ error: 'Invalid tier' });
  }

  const priceId = process.env[config.priceEnv];
  if (!priceId) {
    console.error(`Price env var ${config.priceEnv} not configured`);
    return res.status(500).json({ error: 'Checkout not configured' });
  }

  const siteUrl = process.env.SITE_URL || 'https://4da.ai';

  try {
    const stripe = getStripe();
    const session = await stripe.checkout.sessions.create({
      mode: config.mode,
      payment_method_types: ['card'],
      line_items: [{ price: priceId, quantity: 1 }],
      customer_creation: config.mode === 'payment' ? 'always' : undefined,
      metadata: config.metadata,
      // {CHECKOUT_SESSION_ID} is replaced by Stripe with the real session ID
      success_url: `${siteUrl}/streets/activate?session_id={CHECKOUT_SESSION_ID}`,
      cancel_url: `${siteUrl}/streets`,
    });

    return res.status(200).json({ url: session.url });
  } catch (err) {
    console.error('Checkout session creation failed:', err.message);
    return res.status(500).json({ error: 'Failed to create checkout session' });
  }
}
