// Cloudflare Pages Function: Email Notification Signup
//
// Stores subscriber emails as Stripe customers with notify metadata.
// Ported from the Vercel serverless handler (api/notify.js).
//
// Secrets/vars (Cloudflare Pages -> Settings -> Environment variables):
//   STRIPE_SECRET_KEY  — Stripe secret key (sk_live_... or sk_test_...)
//   ENVIRONMENT        — "production" in prod; anything else enables localhost CORS

import Stripe from 'stripe';

const BASE_ORIGINS = ['https://4da.ai', 'https://www.4da.ai'];

function corsHeaders(request, env) {
  const headers = new Headers();
  const origin = request.headers.get('origin');
  const allowed =
    env.ENVIRONMENT !== 'production'
      ? [...BASE_ORIGINS, 'http://localhost:4444', 'http://localhost:1420', 'http://localhost:8080']
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

  let email;
  try {
    const body = await request.json();
    email = body?.email;
  } catch {
    return json({ error: 'Invalid request body' }, 400, headers);
  }

  if (!email || typeof email !== 'string' || !email.includes('@')) {
    return json({ error: 'Valid email required' }, 400, headers);
  }

  if (!env.STRIPE_SECRET_KEY) {
    return json({ error: 'Service not configured' }, 500, headers);
  }

  try {
    const stripe = new Stripe(env.STRIPE_SECRET_KEY, { httpClient: Stripe.createFetchHttpClient() });

    const existing = await stripe.customers.list({ email: email.toLowerCase(), limit: 1 });
    if (existing.data.length > 0) {
      await stripe.customers.update(existing.data[0].id, {
        metadata: { ...existing.data[0].metadata, notify_updates: 'true', notify_source: '4da-landing' },
      });
    } else {
      await stripe.customers.create({
        email: email.toLowerCase(),
        metadata: { notify_updates: 'true', notify_source: '4da-landing' },
      });
    }

    return json({ ok: true }, 200, headers);
  } catch (err) {
    console.error('Notify error:', err.message);
    return json({ error: 'Failed to subscribe' }, 500, headers);
  }
}
