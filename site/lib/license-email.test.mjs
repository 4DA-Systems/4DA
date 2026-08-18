// Tests for licence email DELIVERY at purchase/renewal.
//
// The defect these exist for: `handleCheckoutCompleted` minted a key into Stripe
// metadata and emailed nothing, so the success page rendering it was the ONLY
// delivery a buyer ever got. A buyer who closed the tab inside the webhook window
// — the page retries 4 times over ~8s — had no key, and the recovery fallback was
// unprovisioned and answered 503. One page load was the whole delivery mechanism
// for a paid product.
//
// Two properties matter more than the happy path and are pinned hardest here:
//   1. delivery NEVER throws — it runs off a paid webhook, and a throw means
//      Stripe retries, and every retry MINTS ANOTHER KEY;
//   2. an unprovisioned mailer is LOUD, not silent — the recovery path's 503 was
//      correct behaviour that nobody could see, which is why it sat dead.
//
// Dependency-free `node --test`, like entitlement.test.mjs: site/ has no
// installed dependency tree in CI, so anything importing `stripe` could not run.

import test from 'node:test';
import assert from 'node:assert/strict';

import { deliverLicenseEmail, isRecoveryEmailConfigured } from './recovery-email.js';

const CONFIGURED = { RESEND_API_KEY: 'test-key', RESEND_FROM_EMAIL: '4DA <licenses@4da.ai>' };
const KEY = '4DA-eyJ0aWVyIjoic2lnbmFsIn0.c2ln';

/**
 * Swap global fetch for a recorder. Returns { calls, restore }.
 * @param {number|Error} outcome HTTP status to answer, or an Error to throw.
 */
function stubFetch(outcome = 200) {
  const calls = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init) => {
    calls.push({ url, init, body: JSON.parse(init.body) });
    if (outcome instanceof Error) throw outcome;
    return { ok: outcome >= 200 && outcome < 300, status: outcome, text: async () => 'stub body' };
  };
  return { calls, restore: () => { globalThis.fetch = original; } };
}

/** Capture console output so the "is it loud?" assertions can inspect it. */
function captureConsole() {
  const errors = [];
  const logs = [];
  const origErr = console.error;
  const origLog = console.log;
  console.error = (...a) => errors.push(a.join(' '));
  console.log = (...a) => logs.push(a.join(' '));
  return { errors, logs, restore: () => { console.error = origErr; console.log = origLog; } };
}

test('a purchase mails the key to the buyer', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    const result = await deliverLicenseEmail(
      CONFIGURED, 'buyer@example.com', KEY, 'signal', '2027-01-01T00:00:00.000Z', 'purchase',
    );
    assert.equal(result, 'sent');
    assert.equal(f.calls.length, 1);
    assert.equal(f.calls[0].url, 'https://api.resend.com/emails');
    assert.equal(f.calls[0].body.to, 'buyer@example.com');
    assert.equal(f.calls[0].body.from, CONFIGURED.RESEND_FROM_EMAIL);
    assert.ok(f.calls[0].body.text.includes(KEY), 'the key is in the plaintext part');
    assert.ok(f.calls[0].body.html.includes(KEY), 'and in the HTML part');
  } finally { f.restore(); c.restore(); }
});

test('the purchase footer does not accuse the buyer of a recovery request', async () => {
  // A confirmation that reads "someone asked to recover your licence" lands as an
  // account-compromise warning at the happiest moment of the funnel.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { text, html, subject } = f.calls[0].body;
    assert.ok(!text.includes('asked 4DA to recover'), 'purchase text must not use recovery wording');
    assert.ok(!html.includes('asked 4DA to recover'), 'purchase HTML must not use recovery wording');
    assert.ok(text.includes('you purchased 4DA Signal'), 'it explains why it arrived');
    assert.match(subject, /Signal licence key/);
  } finally { f.restore(); c.restore(); }
});

test('a renewal says the previous key is being replaced', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'renewal');
    const { text, subject } = f.calls[0].body;
    assert.ok(text.includes('replaces the previous one'), 'a silent key swap must be explained');
    assert.match(subject, /renewed/);
  } finally { f.restore(); c.restore(); }
});

test('an UNPROVISIONED mailer is loud and sends nothing', async () => {
  // This is the state production was actually in: correct-but-invisible.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    const result = await deliverLicenseEmail({}, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    assert.equal(result, 'not_configured');
    assert.equal(f.calls.length, 0, 'no network call without credentials');
    assert.equal(c.errors.length, 1, 'exactly one error, not a silent skip');
    assert.match(c.errors[0], /RESEND_API_KEY/, 'the log names the missing variable');
    assert.match(c.errors[0], /success page is their only/, 'and states the consequence');
  } finally { f.restore(); c.restore(); }
});

test('a Resend HTTP failure NEVER throws — a throw would mint a second key', async () => {
  // The no-throw contract: a throw here returns non-2xx to Stripe, Stripe retries,
  // and generateAndStoreLicense is not idempotent, so the retry issues another
  // valid licence with a fresh expiry.
  const f = stubFetch(500);
  const c = captureConsole();
  try {
    const result = await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    assert.equal(result, 'error');
    assert.equal(c.errors.length, 1);
  } finally { f.restore(); c.restore(); }
});

test('a network exception NEVER throws either', async () => {
  const f = stubFetch(new Error('socket hang up'));
  const c = captureConsole();
  try {
    const result = await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    assert.equal(result, 'error');
  } finally { f.restore(); c.restore(); }
});

test('a missing address or key is refused rather than sent blank', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    assert.equal(await deliverLicenseEmail(CONFIGURED, '', KEY, 'signal', null, 'purchase'), 'error');
    assert.equal(await deliverLicenseEmail(CONFIGURED, 'a@b.co', '', 'signal', null, 'purchase'), 'error');
    assert.equal(f.calls.length, 0, 'nothing is sent');
  } finally { f.restore(); c.restore(); }
});

test('a Date expiry renders instead of silently killing the email', async () => {
  // The webhook path holds a Date (generateAndStoreLicense returns new Date(...))
  // while recovery reads an ISO string from Stripe metadata. `.slice()` on a Date
  // throws, and the no-throw contract would swallow that into 'error' — a
  // silently unsent licence email, i.e. exactly the failure class this module
  // exists to end. Both shapes must render.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    const asDate = new Date('2027-03-04T05:06:07.000Z');
    assert.equal(
      await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', asDate, 'purchase'),
      'sent',
      'a Date must not fail the send',
    );
    assert.ok(f.calls[0].body.text.includes('Valid until: 2027-03-04'), 'Date rendered as a date');

    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', '2027-03-04T05:06:07.000Z', 'purchase');
    assert.ok(f.calls[1].body.text.includes('Valid until: 2027-03-04'), 'ISO string renders identically');
  } finally { f.restore(); c.restore(); }
});

test('a junk expiry is omitted rather than rendered as junk', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', {}, 'purchase');
    assert.equal(f.calls.length, 1, 'still sends — a bad expiry must not cost the buyer their key');
    assert.ok(!f.calls[0].body.text.includes('Valid until'), 'and simply omits the line');
  } finally { f.restore(); c.restore(); }
});

test('the activate button is an https link, NOT a 4da:// deep link', async () => {
  // The defect: Gmail strips custom-scheme hrefs outright, in the browser and in
  // its mobile apps, so `4da://activate?key=...` rendered as a button with no href
  // and did nothing when clicked -- in the most used mail client there is. Verified
  // dead against a real delivered email before this changed. The app was never at
  // fault; the `4da` scheme is registered and handled.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html, text } = f.calls[0].body;
    assert.ok(html.includes('https://4da.ai/activate#key='), 'the button must point at the https bridge');
    assert.ok(text.includes('https://4da.ai/activate#key='), 'and so must the plaintext part');
    assert.ok(!html.includes('href="4da://'), 'a custom-scheme href would be stripped by Gmail');
    assert.ok(!text.includes('4da://'), 'the plaintext link must be clickable too');
  } finally { f.restore(); c.restore(); }
});

test('the key rides in the FRAGMENT so it never reaches a server', async () => {
  // A fragment is not transmitted in the request, so the licence key stays out of
  // Cloudflare's logs and out of any Referer header. `?key=` would leak it to both.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html, text } = f.calls[0].body;
    for (const [label, part] of [['html', html], ['text', text]]) {
      assert.ok(!part.includes('/activate?key='), `${label} must not put the key in the query string`);
      const url = part.match(/https:\/\/4da\.ai\/activate#key=([^\s"<]+)/);
      assert.ok(url, `${label} carries an activate URL`);
      assert.equal(decodeURIComponent(url[1]), KEY, `${label} round-trips the key intact`);
    }
  } finally { f.restore(); c.restore(); }
});

test('a key with URL-hostile characters survives the round trip', async () => {
  // Licence keys are base64 and legitimately contain + and / and =, all of which
  // change meaning unescaped in a URL.
  const f = stubFetch(200);
  const c = captureConsole();
  const gnarly = '4DA-abc+def/ghi==.sig+val/ue==';
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', gnarly, 'signal', null, 'purchase');
    const url = f.calls[0].body.text.match(/https:\/\/4da\.ai\/activate#key=([^\s]+)/);
    assert.ok(url);
    assert.equal(decodeURIComponent(url[1]), gnarly, 'plus, slash and equals all survive');
    assert.ok(!url[1].includes('+'), 'a raw + would decode to a space');
  } finally { f.restore(); c.restore(); }
});

test('the preheader claims the preview line and NEVER contains the key', async () => {
  // The defect, seen in a real inbox: with no preheader the client fills the
  // preview line from the top of the body, so Gmail displayed the raw base64
  // licence key in the message list -- and therefore on lock screens and in
  // notification banners too. Unprofessional, and a small privacy leak.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html } = f.calls[0].body;
    const preheader = html.match(/mso-hide: all;">([^<]*)</);
    assert.ok(preheader, 'a hidden preheader block exists');
    assert.match(preheader[1], /licence key is inside/i, 'it says something purposeful');
    assert.ok(!preheader[1].includes(KEY), 'and it must NEVER carry the key');
    // The key must not appear before the preheader either, or it wins the preview.
    assert.ok(html.indexOf('mso-hide') < html.indexOf(KEY), 'preheader precedes the key');
  } finally { f.restore(); c.restore(); }
});

test('the layout is table-based, not a styled body div', async () => {
  // Outlook on Windows renders through Word: it ignores max-width and most
  // margins on <body>, so the first version arrived full-bleed and ragged for
  // precisely the desktop business audience most likely to buy a dev tool.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html } = f.calls[0].body;
    assert.ok(html.includes('role="presentation"'), 'uses presentation tables');
    assert.ok(!/<body[^>]*max-width/.test(html), 'no max-width on body — Outlook drops it');
    assert.ok(/<meta name="color-scheme" content="light">/.test(html), 'opts out of dark-mode auto-inversion');
  } finally { f.restore(); c.restore(); }
});

test('the sun mark is additive — branding never depends on a blocked image', async () => {
  // Most clients block remote images by default. The 4-sun may enhance the
  // header, but the brand must survive without it: text wordmark always present,
  // alt="" + fixed dimensions so a blocked image is a clean gap (not a broken
  // icon with stray alt text), and NEVER a data: URI — Gmail strips those.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html } = f.calls[0].body;
    assert.match(html, />4DA<\/td>/, 'the wordmark is text, in its own cell');
    const img = html.match(/<img[^>]*>/);
    assert.ok(img, 'the sun mark is present');
    assert.ok(img[0].includes('src="https://4da.ai/email-sun.jpg"'), 'hosted asset, absolute URL');
    assert.ok(img[0].includes('alt=""'), 'decorative: no stray alt text when blocked');
    assert.match(img[0], /width="\d+" height="\d+"/, 'fixed box so a blocked image cannot reflow the header');
    assert.ok(!html.includes('data:image'), 'no data: URI — Gmail strips them');
  } finally { f.restore(); c.restore(); }
});

test('the button colour sits on a td so Outlook still renders a button', async () => {
  // A padded <a> collapses to bare underlined text in Word-rendered Outlook.
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'buyer@example.com', KEY, 'signal', null, 'purchase');
    const { html } = f.calls[0].body;
    assert.match(html, /<td[^>]*bgcolor="#D4AF37"/, 'the gold sits on a table cell');
    assert.ok(html.includes('Activate in 4DA'), 'and the label is present');
  } finally { f.restore(); c.restore(); }
});

test('a renewal is badged differently from a first purchase', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'b@e.co', KEY, 'signal', null, 'purchase');
    await deliverLicenseEmail(CONFIGURED, 'b@e.co', KEY, 'signal', null, 'renewal');
    assert.match(f.calls[0].body.html, /Signal licence<\/td>/, 'purchase badge');
    assert.match(f.calls[1].body.html, /Subscription renewed<\/td>/, 'renewal badge');
    assert.match(f.calls[1].body.html, /replaces your previous key/, 'renewal preheader says so');
  } finally { f.restore(); c.restore(); }
});

test('every email offers a human reply path', async () => {
  const f = stubFetch(200);
  const c = captureConsole();
  try {
    await deliverLicenseEmail(CONFIGURED, 'b@e.co', KEY, 'signal', null, 'purchase');
    assert.match(f.calls[0].body.html, /reply to this email/i);
    assert.match(f.calls[0].body.text, /reply to this email/i);
  } finally { f.restore(); c.restore(); }
});

test('isRecoveryEmailConfigured requires BOTH variables', () => {
  assert.equal(isRecoveryEmailConfigured(CONFIGURED), true);
  assert.equal(isRecoveryEmailConfigured({ RESEND_API_KEY: 'k' }), false);
  assert.equal(isRecoveryEmailConfigured({ RESEND_FROM_EMAIL: 'f' }), false);
  assert.equal(isRecoveryEmailConfigured({}), false);
  assert.equal(isRecoveryEmailConfigured(null), false);
});
