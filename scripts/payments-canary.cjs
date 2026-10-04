#!/usr/bin/env node
/**
 * Payments canary — can a customer actually buy from 4DA right now?
 *
 *   node scripts/payments-canary.cjs              # cheap checks
 *   node scripts/payments-canary.cjs --checkout   # also mint one live Stripe
 *                                                 # Checkout session per plan
 *   node scripts/payments-canary.cjs --json
 *
 * Exit codes: 0 every check passed, 1 at least one failed. A network error is a
 * FAILED check, never a skipped one — "could not reach the payment path" is
 * exactly the condition this exists to report.
 *
 * WHY THIS EXISTS
 * ---------------
 * Every selling path failed silently at least once before anyone looked:
 *   - Signal renewals answered Stripe 200 "not a subscription invoice" for every
 *     renewal once the webhook endpoint moved to API basil+ (#817). Stripe saw
 *     success, so nothing retried and nothing alerted.
 *   - The licence-signing seed could not be proven to match the app's public key
 *     at all until /api/license/health existed (#819). A wrong seed would mint
 *     keys the app rejects while every response stays green.
 *   - The merch store sold to Australia only while the site promised worldwide
 *     shipping; and Printful's "Rest of world" zone silently excluded every
 *     country that has provinces until the zones were listed explicitly.
 *     Printful can re-sync its shipping profiles and undo that.
 * None of these is visible to CI (it cannot read live config) or to Stripe's
 * own failure emails (the responses were 2xx). Only asking production is.
 *
 * Everything here is read-only except --checkout, which creates live Checkout
 * Sessions that are never paid and expire on their own after 24h, and one
 * Shopify cart (carts are anonymous and expire). No email is sent: the recovery
 * probe uses an address that is never a customer, which the endpoint answers
 * with its constant 202 without mailing anything.
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const SITE = 'https://4da.ai';
const STORE = '4da-2.myshopify.com';
const PLANS = ['monthly', 'annual', 'lifetime'];
// The shipping zone regression only shows for countries WITH provinces; a
// public landmark address is enough for Shopify to quote a rate.
const PROVINCE_PROBE = {
  country: 'MX',
  address1: 'Plaza de la Constitucion',
  city: 'Ciudad de Mexico',
  province: 'DF',
  zip: '06000',
};
// Below this many sellable countries the International market has been
// removed or narrowed (it was 235 when set up on 2026-10-04; AU-only was 1).
const MIN_COUNTRIES = 200;
const MIN_PRODUCTS = 14;

/** Read the public Storefront token + API version the live merch page uses. */
function storefrontConfig(root = path.join(__dirname, '..')) {
  const src = fs.readFileSync(path.join(root, 'site', 'src', 'merch.njk'), 'utf8');
  const token = (src.match(/var TOKEN = '([^']+)'/) || [])[1];
  const apiVersion = (src.match(/var API_VER = '([^']+)'/) || [])[1];
  if (!token || !apiVersion) throw new Error('could not read TOKEN/API_VER from site/src/merch.njk');
  return { token, apiVersion };
}

async function readJson(res) {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { _raw: text.slice(0, 200) };
  }
}

/**
 * Run every check. `fetch` is injected so the classification logic is testable
 * without the network.
 *
 * @returns {Promise<Array<{name:string, ok:boolean, detail:string}>>}
 */
class Inconclusive extends Error {}
const THROTTLE_RETRIES = 3;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function runChecks({ fetch, withCheckout = false, storefront, throttleDelayMs = 15000 }) {
  const results = [];
  const check = async (name, fn) => {
    try {
      const detail = await fn();
      results.push({ name, ok: true, detail: detail || 'ok' });
    } catch (err) {
      // Shopify rate-limits cartCreate per IP. A throttle says nothing about
      // whether customers can buy, so it is reported, never alarmed on.
      if (err instanceof Inconclusive) {
        results.push({ name, ok: true, inconclusive: true, detail: `INCONCLUSIVE: ${err.message}` });
        return;
      }
      results.push({ name, ok: false, detail: err && err.message ? err.message : String(err) });
    }
  };
  const fail = (msg) => {
    throw new Error(msg);
  };

  await check('licence signing key matches the app', async () => {
    const res = await fetch(`${SITE}/api/license/health`);
    const body = await readJson(res);
    if (res.status !== 200 || body.ok !== true || body.signing_key?.matches_app !== true) {
      fail(`HTTP ${res.status} ${JSON.stringify(body).slice(0, 300)}`);
    }
    return 'matches_app=true';
  });

  await check('webhook endpoint verifies signatures', async () => {
    // An unsigned POST must be refused as a bad signature. A 500 "Webhook secret
    // not configured" (or any other answer) means Stripe deliveries are failing.
    const res = await fetch(`${SITE}/api/license/activate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: '{}',
    });
    const body = await readJson(res);
    if (res.status !== 400 || body.error !== 'Invalid signature') {
      fail(`expected 400 Invalid signature, got HTTP ${res.status} ${JSON.stringify(body).slice(0, 200)}`);
    }
    return '400 Invalid signature';
  });

  await check('licence recovery email is provisioned', async () => {
    const res = await fetch(
      `${SITE}/api/license/activate?email=${encodeURIComponent('canary@payments-canary.invalid')}`,
    );
    if (res.status !== 202) {
      const body = await readJson(res);
      fail(`expected 202, got HTTP ${res.status} ${JSON.stringify(body).slice(0, 200)}`);
    }
    return '202';
  });

  for (const page of ['/signal', '/signal/success', '/activate', '/merch', '/download']) {
    await check(`page ${page} serves`, async () => {
      const res = await fetch(`${SITE}${page}`, { redirect: 'follow' });
      if (res.status !== 200) fail(`HTTP ${res.status}`);
      return '200';
    });
  }

  await check('Windows installer download resolves', async () => {
    const res = await fetch(`${SITE}/download/win/`, { redirect: 'follow', method: 'HEAD' });
    if (res.status !== 200) fail(`HTTP ${res.status} at ${res.url}`);
    return `200 ${(res.url || '').split('/').pop() || 'resolved'}`;
  });

  if (withCheckout) {
    for (const plan of PLANS) {
      await check(`Stripe checkout opens (${plan})`, async () => {
        const res = await fetch(`${SITE}/api/signal/checkout`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', Origin: SITE },
          body: JSON.stringify({ plan }),
        });
        const body = await readJson(res);
        const url = body.url || '';
        if (res.status !== 200) fail(`HTTP ${res.status} ${JSON.stringify(body).slice(0, 200)}`);
        // cs_live_ — a test-mode key swapped into production would mint cs_test_
        // sessions that take no real money.
        let parsed = null;
        try {
          parsed = new URL(url);
        } catch {
          parsed = null;
        }
        if (!parsed || parsed.protocol !== 'https:' || parsed.hostname !== 'checkout.stripe.com' || !parsed.pathname.includes('/cs_live_')) {
          fail(`not a live Stripe Checkout URL: ${url.slice(0, 80)}`);
        }
        return 'cs_live session';
      });
    }
  }

  if (storefront) {
    const gql = async (query, variables) => {
      for (let attempt = 0; attempt < THROTTLE_RETRIES; attempt++) {
        const res = await fetch(`https://${STORE}/api/${storefront.apiVersion}/graphql.json`, {
          method: 'POST',
          headers: {
            'X-Shopify-Storefront-Access-Token': storefront.token,
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ query, variables }),
        });
        const body = await readJson(res);
        const throttled = (body.errors || []).some((e) => e?.extensions?.code === 'THROTTLED');
        if (throttled) {
          if (attempt < THROTTLE_RETRIES - 1) await sleep(throttleDelayMs * (attempt + 1));
          continue;
        }
        if (res.status !== 200 || body.errors) {
          fail(`Storefront API HTTP ${res.status} ${JSON.stringify(body.errors || body).slice(0, 200)}`);
        }
        return body.data;
      }
      throw new Inconclusive(`Storefront API still throttled after ${THROTTLE_RETRIES} attempts`);
    };

    let teeVariant = null;
    await check('merch products are for sale', async () => {
      const data = await gql(
        '{products(first:50){nodes{title availableForSale variants(first:1){nodes{id}}}}}',
      );
      const nodes = data.products.nodes;
      const sellable = nodes.filter((p) => p.availableForSale);
      if (sellable.length < MIN_PRODUCTS) fail(`${sellable.length}/${nodes.length} sellable, expected >= ${MIN_PRODUCTS}`);
      teeVariant = (sellable.find((p) => /Tee/.test(p.title)) || sellable[0]).variants.nodes[0].id;
      return `${sellable.length} sellable`;
    });

    await check('merch sells internationally', async () => {
      const data = await gql('{localization{availableCountries{isoCode}}}');
      const n = data.localization.availableCountries.length;
      if (n < MIN_COUNTRIES) fail(`only ${n} countries can check out (expected >= ${MIN_COUNTRIES})`);
      return `${n} countries`;
    });

    await check('merch ships to a province country (Printful zones)', async () => {
      if (!teeVariant) fail('no sellable variant to test with');
      const { country, ...address } = PROVINCE_PROBE;
      const data = await gql(
        `mutation($l:[CartLineInput!],$b:CartBuyerIdentityInput){cartCreate(input:{lines:$l,buyerIdentity:$b}){cart{deliveryGroups(first:5){nodes{deliveryOptions{title}}}} userErrors{message}}}`,
        {
          l: [{ merchandiseId: teeVariant, quantity: 1 }],
          b: { countryCode: country, deliveryAddressPreferences: [{ deliveryAddress: { ...address, country } }] },
        },
      );
      const options = (data.cartCreate.cart?.deliveryGroups.nodes || []).flatMap((g) => g.deliveryOptions);
      if (options.length === 0) {
        fail(`no delivery option to ${country} — Printful shipping profiles may have re-synced to "Rest of world"`);
      }
      return options.map((o) => o.title).join(', ');
    });
  }

  return results;
}

function formatReport(results) {
  const lines = results.map((r) => `${r.ok ? (r.inconclusive ? 'WARN' : 'PASS') : 'FAIL'}  ${r.name} — ${r.detail}`);
  const failed = results.filter((r) => !r.ok).length;
  lines.push('');
  lines.push(failed === 0 ? `All ${results.length} payment checks passed.` : `${failed} of ${results.length} payment checks FAILED.`);
  return lines.join('\n');
}

async function main() {
  const args = process.argv.slice(2);
  const results = await runChecks({
    fetch: globalThis.fetch,
    withCheckout: args.includes('--checkout'),
    storefront: storefrontConfig(),
  });
  if (args.includes('--json')) {
    process.stdout.write(JSON.stringify(results, null, 2) + '\n');
  } else {
    process.stdout.write(formatReport(results) + '\n');
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    fs.appendFileSync(
      process.env.GITHUB_STEP_SUMMARY,
      `## Payments canary\n\n\`\`\`\n${formatReport(results)}\n\`\`\`\n`,
    );
  }
  if (process.env.CANARY_REPORT_FILE) {
    fs.writeFileSync(process.env.CANARY_REPORT_FILE, formatReport(results));
  }
  process.exit(results.every((r) => r.ok) ? 0 : 1);
}

if (require.main === module) {
  main().catch((err) => {
    console.error(`payments canary crashed: ${err && err.stack ? err.stack : err}`);
    process.exit(1);
  });
}

module.exports = { runChecks, formatReport, storefrontConfig, MIN_COUNTRIES, MIN_PRODUCTS };
