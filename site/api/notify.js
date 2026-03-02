// Vercel Serverless: Email Notification Signup
//
// Stores subscriber emails as Stripe customers with notify metadata.
// Reuses existing STRIPE_SECRET_KEY — zero new dependencies.
// Environment variables required:
//   STRIPE_SECRET_KEY — Stripe secret key (sk_live_... or sk_test_...)

import Stripe from 'stripe';

const ALLOWED_ORIGINS = [
  'https://4da.ai',
  'https://www.4da.ai',
  ...(process.env.VERCEL_ENV !== 'production'
    ? ['http://localhost:4444', 'http://localhost:1420', 'http://localhost:8080']
    : []),
];

function setCors(req, res) {
  const origin = req.headers.origin;
  if (origin && ALLOWED_ORIGINS.includes(origin)) {
    res.setHeader('Access-Control-Allow-Origin', origin);
    res.setHeader('Vary', 'Origin');
  }
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
}

export default async function handler(req, res) {
  setCors(req, res);

  if (req.method === 'OPTIONS') {
    return res.status(200).end();
  }

  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  const { email } = req.body || {};

  if (!email || typeof email !== 'string' || !email.includes('@')) {
    return res.status(400).json({ error: 'Valid email required' });
  }

  const stripeKey = process.env.STRIPE_SECRET_KEY;
  if (!stripeKey) {
    return res.status(500).json({ error: 'Service not configured' });
  }

  try {
    const stripe = new Stripe(stripeKey);

    // Check if customer already exists
    const existing = await stripe.customers.list({ email: email.toLowerCase(), limit: 1 });

    if (existing.data.length > 0) {
      // Update existing customer with notify flag
      await stripe.customers.update(existing.data[0].id, {
        metadata: { ...existing.data[0].metadata, notify_updates: 'true', notify_source: '4da-landing' },
      });
    } else {
      // Create new customer with notify metadata
      await stripe.customers.create({
        email: email.toLowerCase(),
        metadata: { notify_updates: 'true', notify_source: '4da-landing' },
      });
    }

    return res.status(200).json({ ok: true });
  } catch (err) {
    console.error('Notify error:', err.message);
    return res.status(500).json({ error: 'Failed to subscribe' });
  }
}
