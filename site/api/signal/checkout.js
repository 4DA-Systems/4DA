// Vercel Serverless: Signal Tier Checkout Session Creator
//
// Creates Stripe Checkout sessions for 4DA Signal subscriptions.
// Environment variables required:
//   STRIPE_SECRET_KEY     — Stripe secret key (sk_live_... or sk_test_...)
//   SIGNAL_PRICE_MONTHLY  — Stripe price ID for Signal monthly ($12/mo AUD)
//   SIGNAL_PRICE_ANNUAL   — Stripe price ID for Signal annual ($99/yr AUD)
//   SIGNAL_PRICE_LIFETIME — Stripe price ID for Signal lifetime ($249 AUD one-time)
//   SITE_URL              — Base URL for redirects (e.g. https://4da.ai)

import Stripe from 'stripe';

const PLANS = {
  monthly: {
    priceEnv: 'SIGNAL_PRICE_MONTHLY',
    mode: 'subscription',
    metadata: { streets_tier: 'signal', billing_period: 'monthly' },
  },
  annual: {
    priceEnv: 'SIGNAL_PRICE_ANNUAL',
    mode: 'subscription',
    metadata: { streets_tier: 'signal', billing_period: 'annual' },
  },
  lifetime: {
    priceEnv: 'SIGNAL_PRICE_LIFETIME',
    mode: 'payment',
    metadata: { streets_tier: 'signal', billing_period: 'lifetime' },
  },
};

let _stripe;
function getStripe() {
  if (!_stripe) {
    const key = process.env.STRIPE_SECRET_KEY;
    if (!key) throw new Error('STRIPE_SECRET_KEY not configured');
    _stripe = new Stripe(key);
  }
  return _stripe;
}

const ALLOWED_ORIGINS = [
  'https://4da.ai',
  'https://www.4da.ai',
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

  let body;
  try {
    body = typeof req.body === 'string' ? JSON.parse(req.body) : (req.body || {});
  } catch {
    return res.status(400).json({ error: 'Invalid request body' });
  }

  const { plan } = body;
  const config = PLANS[plan];
  if (!config) {
    return res.status(400).json({ error: 'Invalid plan. Use "monthly", "annual", or "lifetime".' });
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
      metadata: config.metadata,
      success_url: `${siteUrl}/signal/success?session_id={CHECKOUT_SESSION_ID}`,
      cancel_url: `${siteUrl}/signal`,
    });

    return res.status(200).json({ url: session.url });
  } catch (err) {
    console.error('Signal checkout session creation failed:', err.message);
    return res.status(500).json({ error: 'Failed to create checkout session' });
  }
}
