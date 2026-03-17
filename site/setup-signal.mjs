#!/usr/bin/env node
// =============================================================================
// Signal Tier Setup — Creates Stripe Product + Prices
//
// Usage:
//   node site/setup-signal.mjs sk_live_xxxxx    (production)
//   node site/setup-signal.mjs sk_test_xxxxx    (test mode)
//
// What it does:
//   1. Creates "4DA Signal" product in Stripe
//   2. Creates monthly price ($12 AUD/mo)
//   3. Creates annual price ($99 AUD/yr)
//   4. Outputs the env vars to set in Vercel
// =============================================================================

const key = process.argv[2];

if (!key || (!key.startsWith('sk_live_') && !key.startsWith('sk_test_'))) {
  console.error('Usage: node site/setup-signal.mjs <STRIPE_SECRET_KEY>');
  console.error('  Key must start with sk_live_ or sk_test_');
  process.exit(1);
}

const isTest = key.startsWith('sk_test_');
console.log(`\nMode: ${isTest ? 'TEST' : 'PRODUCTION'}`);
console.log('================================================\n');

const Stripe = (await import('stripe')).default;
const stripe = new Stripe(key);

// Check if product already exists
console.log('Checking for existing 4DA Signal product...');
const existingProducts = await stripe.products.search({
  query: "name:'4DA Signal'",
});

let product;
if (existingProducts.data.length > 0) {
  product = existingProducts.data[0];
  console.log(`  Found existing product: ${product.id}`);
} else {
  product = await stripe.products.create({
    name: '4DA Signal',
    description: 'AI-powered developer intelligence. Daily briefings, Developer DNA, score autopsy, signal chains, semantic shifts, natural language search, and more.',
    metadata: { tier: 'signal', app: '4da' },
  });
  console.log(`  Created product: ${product.id}`);
}

// Check for existing prices
console.log('\nChecking for existing prices...');
const existingPrices = await stripe.prices.list({
  product: product.id,
  active: true,
  limit: 10,
});

let monthlyPrice = existingPrices.data.find(
  p => p.recurring?.interval === 'month' && p.unit_amount === 1200 && p.currency === 'aud'
);
let annualPrice = existingPrices.data.find(
  p => p.recurring?.interval === 'year' && p.unit_amount === 9900 && p.currency === 'aud'
);

if (monthlyPrice) {
  console.log(`  Found existing monthly price: ${monthlyPrice.id}`);
} else {
  monthlyPrice = await stripe.prices.create({
    product: product.id,
    unit_amount: 1200, // $12.00 AUD
    currency: 'aud',
    recurring: { interval: 'month' },
    metadata: { plan: 'signal_monthly' },
  });
  console.log(`  Created monthly price: ${monthlyPrice.id} ($12/mo AUD)`);
}

if (annualPrice) {
  console.log(`  Found existing annual price: ${annualPrice.id}`);
} else {
  annualPrice = await stripe.prices.create({
    product: product.id,
    unit_amount: 9900, // $99.00 AUD
    currency: 'aud',
    recurring: { interval: 'year' },
    metadata: { plan: 'signal_annual' },
  });
  console.log(`  Created annual price: ${annualPrice.id} ($99/yr AUD)`);
}

// Check webhook endpoints
console.log('\nChecking webhook endpoints...');
const webhooks = await stripe.webhookEndpoints.list({ limit: 20 });
const signalWebhook = webhooks.data.find(w =>
  w.url.includes('4da.ai') && w.url.includes('/api/streets/activate')
);

if (signalWebhook) {
  console.log(`  Found webhook: ${signalWebhook.url}`);
  console.log(`  Events: ${signalWebhook.enabled_events.join(', ')}`);

  // Ensure all needed events are registered
  const needed = ['checkout.session.completed', 'invoice.paid', 'customer.subscription.deleted'];
  const missing = needed.filter(e => !signalWebhook.enabled_events.includes(e) && !signalWebhook.enabled_events.includes('*'));
  if (missing.length > 0) {
    console.log(`  WARNING: Missing events: ${missing.join(', ')}`);
    console.log('  Update the webhook in Stripe Dashboard to include these events.');
  } else {
    console.log('  All required events registered.');
  }
} else {
  console.log('  WARNING: No webhook endpoint found for 4da.ai/api/streets/activate');
  console.log('  Create one in Stripe Dashboard:');
  console.log('    URL: https://4da.ai/api/streets/activate');
  console.log('    Events: checkout.session.completed, invoice.paid, customer.subscription.deleted');
}

// Output
console.log('\n================================================');
console.log('SETUP COMPLETE\n');
console.log('Add these environment variables to Vercel:\n');
console.log(`  SIGNAL_PRICE_MONTHLY=${monthlyPrice.id}`);
console.log(`  SIGNAL_PRICE_ANNUAL=${annualPrice.id}`);
console.log('\nTo set them via Vercel CLI:');
console.log(`  npx vercel env add SIGNAL_PRICE_MONTHLY production <<< "${monthlyPrice.id}"`);
console.log(`  npx vercel env add SIGNAL_PRICE_ANNUAL production <<< "${annualPrice.id}"`);
console.log('\nOr set them in Vercel Dashboard:');
console.log('  https://vercel.com → Project → Settings → Environment Variables');
console.log('\nAlready set (verify these exist):');
console.log('  STRIPE_SECRET_KEY');
console.log('  STRIPE_WEBHOOK_SECRET');
console.log('  LICENSE_PRIVATE_KEY_HEX');
console.log('  SITE_URL=https://4da.ai');
console.log('\n================================================');
