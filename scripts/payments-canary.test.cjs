'use strict';
// Classification tests for the payments canary: every failure mode it exists to
// catch must turn a check red, and a healthy production must be all green.
// Network-free — `fetch` is faked.

const test = require('node:test');
const assert = require('node:assert/strict');

const { runChecks, formatReport, MIN_COUNTRIES } = require('./payments-canary.cjs');

const APP_KEY = '084dc1b1b9549bf0ddff11db9186cb623ceb9d72831fbf2e6f01db160388f9d6';

function response(status, body, url = '') {
  const text = typeof body === 'string' ? body : JSON.stringify(body);
  return { status, url, text: async () => text };
}

/** A fake production. `overrides` replace individual answers by key. */
function fakeFetch(overrides = {}) {
  const calls = [];
  const healthy = {
    health: () => response(200, { ok: true, signing_key: { matches_app: true, derived_public_key: APP_KEY } }),
    webhook: () => response(400, { error: 'Invalid signature' }),
    recovery: () => response(202, { delivery: 'email' }),
    page: () => response(200, '<html>'),
    download: () => response(200, '', 'https://github.com/x/releases/download/v1.0.2/4DA_1.0.2_x64-setup.exe'),
    checkout: (plan) => response(200, { url: `https://checkout.stripe.com/c/pay/cs_live_${plan}#x` }),
    products: () =>
      response(200, {
        data: {
          products: {
            nodes: Array.from({ length: 14 }, (_, i) => ({
              title: i === 0 ? '4DA Logo Tee' : `Item ${i}`,
              availableForSale: true,
              variants: { nodes: [{ id: `gid://v/${i}` }] },
            })),
          },
        },
      }),
    countries: () =>
      response(200, { data: { localization: { availableCountries: Array.from({ length: 235 }, () => ({})) } } }),
    cart: () =>
      response(200, {
        data: { cartCreate: { cart: { deliveryGroups: { nodes: [{ deliveryOptions: [{ title: 'Worldwide Flat Rate' }] }] } }, userErrors: [] } },
      }),
  };
  const answer = { ...healthy, ...overrides };
  const fetch = async (url, init = {}) => {
    calls.push({ url, init });
    if (url.endsWith('/api/license/health')) return answer.health();
    if (url.endsWith('/api/license/activate') && init.method === 'POST') return answer.webhook();
    if (url.includes('/api/license/activate?email=')) return answer.recovery();
    if (url.endsWith('/download/win/')) return answer.download();
    if (url.endsWith('/api/signal/checkout')) return answer.checkout(JSON.parse(init.body).plan);
    if (new URL(url).hostname === '4da-2.myshopify.com') {
      const q = JSON.parse(init.body).query;
      if (q.includes('products(')) return answer.products();
      if (q.includes('localization')) return answer.countries();
      if (q.includes('cartCreate')) return answer.cart();
    }
    return answer.page(url);
  };
  return { fetch, calls };
}

const storefront = { token: 't', apiVersion: '2024-01' };
const failed = (results) => results.filter((r) => !r.ok).map((r) => r.name);

test('a healthy production is all green, including checkout', async () => {
  const { fetch } = fakeFetch();
  const results = await runChecks({ fetch, withCheckout: true, storefront, throttleDelayMs: 0 });
  assert.deepEqual(failed(results), []);
  assert.ok(results.some((r) => r.name === 'Stripe checkout opens (lifetime)'));
  assert.match(formatReport(results), /All \d+ payment checks passed/);
});

test('checkout is only exercised when asked (it mints live sessions)', async () => {
  const { fetch, calls } = fakeFetch();
  await runChecks({ fetch, storefront, throttleDelayMs: 0 });
  assert.equal(calls.filter((c) => c.url.endsWith('/api/signal/checkout')).length, 0);
});

test('a signing key that does not match the app fails', async () => {
  const { fetch } = fakeFetch({
    health: () => response(503, { ok: false, signing_key: { matches_app: false, derived_public_key: 'ab'.repeat(32) } }),
  });
  const results = await runChecks({ fetch, storefront, throttleDelayMs: 0 });
  assert.deepEqual(failed(results), ['licence signing key matches the app']);
});

test('a webhook with no secret configured fails', async () => {
  const { fetch } = fakeFetch({ webhook: () => response(500, { error: 'Webhook secret not configured' }) });
  assert.deepEqual(failed(await runChecks({ fetch, storefront, throttleDelayMs: 0 })), ['webhook endpoint verifies signatures']);
});

test('unprovisioned recovery email fails', async () => {
  const { fetch } = fakeFetch({ recovery: () => response(503, { reason: 'recovery_email_unavailable' }) });
  assert.deepEqual(failed(await runChecks({ fetch, storefront, throttleDelayMs: 0 })), ['licence recovery email is provisioned']);
});

test('a test-mode Stripe key in production fails checkout', async () => {
  const { fetch } = fakeFetch({ checkout: (plan) => response(200, { url: `https://checkout.stripe.com/c/pay/cs_test_${plan}` }) });
  const results = await runChecks({ fetch, withCheckout: true, storefront, throttleDelayMs: 0 });
  assert.deepEqual(failed(results), [
    'Stripe checkout opens (monthly)',
    'Stripe checkout opens (annual)',
    'Stripe checkout opens (lifetime)',
  ]);
});

test('an unconfigured price fails just that plan', async () => {
  const { fetch } = fakeFetch({
    checkout: (plan) =>
      plan === 'lifetime'
        ? response(500, { error: 'Checkout not configured' })
        : response(200, { url: `https://checkout.stripe.com/c/pay/cs_live_${plan}` }),
  });
  assert.deepEqual(failed(await runChecks({ fetch, withCheckout: true, storefront, throttleDelayMs: 0 })), [
    'Stripe checkout opens (lifetime)',
  ]);
});

test('the store reverting to Australia-only fails', async () => {
  const { fetch } = fakeFetch({
    countries: () => response(200, { data: { localization: { availableCountries: [{}] } } }),
  });
  const results = await runChecks({ fetch, storefront, throttleDelayMs: 0 });
  assert.deepEqual(failed(results), ['merch sells internationally']);
  assert.ok(MIN_COUNTRIES > 1);
});

test('Printful zones reverting (no rate to a province country) fails', async () => {
  const { fetch } = fakeFetch({
    cart: () => response(200, { data: { cartCreate: { cart: { deliveryGroups: { nodes: [] } }, userErrors: [] } } }),
  });
  assert.deepEqual(failed(await runChecks({ fetch, storefront, throttleDelayMs: 0 })), [
    'merch ships to a province country (Printful zones)',
  ]);
});

test('a Shopify throttle is reported as inconclusive, never as an outage', async () => {
  let cartCalls = 0;
  const { fetch } = fakeFetch({
    cart: () => {
      cartCalls++;
      return response(200, { errors: [{ message: 'Throttled', extensions: { code: 'THROTTLED' } }], data: { cartCreate: { cart: null } } });
    },
  });
  const results = await runChecks({ fetch, storefront, throttleDelayMs: 0 });
  assert.deepEqual(failed(results), []);
  const probe = results.find((r) => r.name.startsWith('merch ships'));
  assert.equal(probe.inconclusive, true);
  assert.equal(cartCalls, 3);
  assert.match(formatReport(results), /WARN  merch ships/);
});

test('an unreachable site is a failure, not a skip', async () => {
  const { fetch: base } = fakeFetch();
  const fetch = async (url, init) => {
    if (new URL(url).hostname === '4da.ai') throw new Error('getaddrinfo ENOTFOUND 4da.ai');
    return base(url, init);
  };
  const results = await runChecks({ fetch, storefront, throttleDelayMs: 0 });
  assert.ok(failed(results).includes('licence signing key matches the app'));
  assert.match(results[0].detail, /ENOTFOUND/);
});
