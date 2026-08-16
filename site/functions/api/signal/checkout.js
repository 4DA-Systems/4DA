// Cloudflare Pages Function: Signal Tier Checkout Session Creator
//
// Creates Stripe Checkout sessions for 4DA Signal subscriptions.
// Ported from the Vercel serverless handler (api/signal/checkout.js).
//
// Secrets/vars (Cloudflare Pages -> Settings -> Environment variables):
//   STRIPE_SECRET_KEY     — Stripe secret key (sk_live_... or sk_test_...)
//   SIGNAL_PRICE_MONTHLY  — Stripe price ID for Signal monthly ($12/mo AUD)
//   SIGNAL_PRICE_ANNUAL   — Stripe price ID for Signal annual ($99/yr AUD)
//   SIGNAL_PRICE_LIFETIME — Stripe price ID for Signal lifetime ($299 AUD one-time)
//   SIGNAL_LAUNCH_COUPON  — Optional, launch-day only: Stripe coupon ID auto-applied to
//                           lifetime checkouts while the coupon is valid; unset = no discount
//   SITE_URL              — Base URL for redirects (e.g. https://4da.ai)
//   ENVIRONMENT           — "production" in prod; anything else enables localhost CORS

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

const BASE_ORIGINS = ['https://4da.ai', 'https://www.4da.ai'];

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
  headers.set('Access-Control-Allow-Methods', 'POST, OPTIONS');
  headers.set('Access-Control-Allow-Headers', 'Content-Type');
  return headers;
}

function json(body, status, headers) {
  headers.set('Content-Type', 'application/json');
  return new Response(JSON.stringify(body), { status, headers });
}

export async function onRequest({ request, env }) {
  const headers = corsHeaders(request, env);

  if (request.method === 'OPTIONS') return new Response(null, { status: 200, headers });
  if (request.method !== 'POST') return json({ error: 'Method not allowed' }, 405, headers);

  let plan;
  try {
    const body = await request.json();
    plan = body?.plan;
  } catch {
    return json({ error: 'Invalid request body' }, 400, headers);
  }

  const config = PLANS[plan];
  if (!config) {
    return json({ error: 'Invalid plan. Use "monthly", "annual", or "lifetime".' }, 400, headers);
  }

  const priceId = env[config.priceEnv];
  if (!priceId) {
    console.error(`Price env var ${config.priceEnv} not configured`);
    return json({ error: 'Checkout not configured' }, 500, headers);
  }

  const siteUrl = env.SITE_URL || 'https://4da.ai';

  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });
    const sessionParams = {
      mode: config.mode,
      payment_method_types: ['card'],
      line_items: [{ price: priceId, quantity: 1 }],
      metadata: config.metadata,
      success_url: `${siteUrl}/signal/success?session_id={CHECKOUT_SESSION_ID}`,
      cancel_url: `${siteUrl}/signal`,
    };
    if (config.mode === 'payment') {
      sessionParams.customer_creation = 'always';
      // Launch special: auto-apply the launch coupon while Stripe reports it valid.
      // Stripe flips valid to false once max_redemptions is reached, so the standing
      // price takes over mechanically. A coupon problem never blocks a full-price sale.
      const coupon = env.SIGNAL_LAUNCH_COUPON;
      if (coupon) {
        try {
          const retrieved = await stripe.coupons.retrieve(coupon);
          if (retrieved.valid === true) {
            sessionParams.discounts = [{ coupon }];
          }
        } catch (err) {
          console.error('Launch coupon lookup failed, proceeding at full price:', err.message);
        }
      }
    }
    const session = await stripe.checkout.sessions.create(sessionParams);

    return json({ url: session.url }, 200, headers);
  } catch (err) {
    console.error('Signal checkout session creation failed:', err.message);
    return json({ error: 'Failed to create checkout session' }, 500, headers);
  }
}
