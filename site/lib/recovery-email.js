// Licence email delivery for Cloudflare Pages Functions (Resend).
//
// TWO jobs, deliberately in one module so they share one Resend client, one
// template and one provisioning check:
//
//   1. DELIVERY at purchase/renewal  — `deliverLicenseEmail`
//   2. RECOVERY on request           — `deliverRecoveryEmail`
//
// (1) exists because for a while the ONLY way a buyer ever received their key
// was the success page rendering it. `handleCheckoutCompleted` minted the key
// into Stripe metadata and emailed nothing, so a buyer who closed the tab inside
// the webhook window — the page retries 4 times over ~8s — had no key and no
// working self-serve route, since (2) was unprovisioned and answered 503. One
// page load was the entire delivery mechanism for a paid product. Emailing at
// purchase makes the key durable and demotes recovery to a genuine fallback.
//
// WHY THIS MODULE EXISTS — security fix, 2026-08-14
// -------------------------------------------------
// `GET /api/streets/activate?email=<address>` used to return the licence key for
// that address directly in the HTTP response body. The `email` parameter is
// caller-supplied and was never verified against anything, so a single
// unauthenticated request yielded a full Ed25519-signed licence key for any
// address an attacker knew or guessed. Those keys are verified OFFLINE by the
// desktop app against an embedded public key, so a stolen key keeps working with
// no further server contact — there is nothing to revoke it with. The 200-vs-404
// split was also a customer-list oracle ("is this person a paying subscriber?").
//
// The fix keeps recovery working without ever handing the key to an unverified
// caller: we mail the key to the address ON FILE. Control of the mailbox becomes
// the implicit authentication factor, and the HTTP response is constant, so the
// oracle disappears too.
//
// Abuse surface, deliberately bounded: we only ever send to an address that is
// ALREADY a Stripe customer holding a licence. The endpoint therefore cannot be
// used to mail arbitrary third parties — at worst an existing customer's inbox
// can be spammed. See the rate-limit follow-up note in activate.js.
//
// Config (Cloudflare Pages -> Settings -> Environment variables):
//   RESEND_API_KEY    — Resend API key (same provider paddle-webhook/ already uses)
//   RESEND_FROM_EMAIL — e.g. "4DA <licenses@4da.ai>"; domain must be verified in Resend

// `meta` resolves the signal_*/streets_* namespace. entitlement.js imports
// nothing, so this stays a one-way dependency with no cycle.
import { isRevoked, meta } from './entitlement.js';
import { renderShell, emailButton, keyPanel, BRAND, FONT_STACK } from './email-shell.js';

const RESEND_ENDPOINT = 'https://api.resend.com/emails';

/**
 * True when outbound licence mail is provisioned. When false the RECOVERY path
 * MUST fail honestly (tell the user to contact support) rather than fall back to
 * returning the key — returning the key is the vulnerability this replaced.
 *
 * The DELIVERY path treats it differently: an unprovisioned mailer must never
 * fail a paid webhook, so delivery logs loudly and the success page remains the
 * fallback. See `deliverLicenseEmail`.
 */
export function isRecoveryEmailConfigured(env) {
  return Boolean(env?.RESEND_API_KEY && env?.RESEND_FROM_EMAIL);
}

/**
 * Cheap shape check. Runs BEFORE any Stripe call so a 400 depends only on the
 * submitted string, never on whether that string belongs to a customer (which
 * would reopen the oracle).
 */
export function isPlausibleEmail(value) {
  return (
    typeof value === 'string' &&
    value.length >= 3 &&
    value.length <= 254 &&
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
  );
}

// ---------------------------------------------------------------------------
// Message bodies
// ---------------------------------------------------------------------------

// The footer has to match WHY the mail arrived. A purchase confirmation that
// says "someone asked to recover your licence" reads as an account-compromise
// warning on the happiest moment of the funnel, and a recovery mail that omits
// the "wasn't you?" line loses the only signal that someone probed the address.
const ENTITY_HTML = `
                <p style="margin: 14px 0 0; font-family: ${FONT_STACK}; font-size: 12px; line-height: 1.6; color: ${BRAND.faint};">
                  4DA Systems Pty Ltd &middot; ACN 696 078 841<br>
                  Questions? Just reply to this email &mdash; it reaches a person.
                </p>`;
const ENTITY_TEXT = [
  '— 4DA Systems Pty Ltd (ACN 696 078 841)',
  'Questions? Just reply to this email — it reaches a person.',
].join('\n');

// The inbox preview line. Without one the client fills it from the top of the
// body, which for the licence email meant displaying the raw base64 KEY in the
// preview, on lock screens and in notification banners. Never put the key here.
const PREHEADER = {
  recovery: 'Your 4DA Signal licence key is inside, as you requested.',
  purchase: 'Your 4DA Signal licence key is inside. Keep this email — it is your permanent copy.',
  renewal: 'Your renewed 4DA Signal licence key is inside. It replaces your previous key.',
};

const REASON_HTML = {
  recovery: `You are receiving this because someone asked 4DA to recover the licence for this
    address. If that wasn't you, no action is needed — nothing about your account changed.`,
  purchase: `You are receiving this because you purchased 4DA Signal. Keep this email — it is
    your own copy of the key, so you never depend on the browser tab you bought in.`,
  renewal: `You are receiving this because your 4DA Signal subscription renewed. The key above
    replaces the previous one; the old key stops working at its original expiry.`,
};

const REASON_TEXT = {
  recovery: [
    'You are receiving this because someone asked 4DA to recover the licence for',
    "this address. If that wasn't you, no action is needed — nothing about your",
    'account changed.',
  ],
  purchase: [
    'You are receiving this because you purchased 4DA Signal. Keep this email —',
    'it is your own copy of the key, so you never depend on the browser tab you',
    'bought in.',
  ],
  renewal: [
    'You are receiving this because your 4DA Signal subscription renewed. The key',
    'above replaces the previous one; the old key stops working at its original',
    'expiry.',
  ],
};

function footerHtml(context) {
  // The shell draws the divider now, so this is content only.
  return `                <p style="margin: 0; font-family: ${FONT_STACK}; font-size: 13px; line-height: 1.65; color: ${BRAND.muted};">
                  ${REASON_HTML[context] || REASON_HTML.recovery}
                </p>${ENTITY_HTML}`;
}

const SUBJECT = {
  recovery: 'Your 4DA licence key',
  purchase: 'Your 4DA Signal licence key',
  renewal: 'Your renewed 4DA Signal licence key',
};

function footerText(context) {
  return ['', ...(REASON_TEXT[context] || REASON_TEXT.recovery), '', ENTITY_TEXT].join('\n');
}

/**
 * Render `expiresAt` as YYYY-MM-DD, accepting a Date OR an ISO string.
 *
 * The two callers genuinely differ: the webhook path holds a `Date`
 * (`generateAndStoreLicense` returns `new Date(...)`), while the recovery path
 * reads an ISO string out of Stripe metadata. `.slice()` on a Date throws, and
 * `deliverLicenseEmail`'s no-throw contract would swallow that into
 * "return 'error'" — i.e. a silently unsent licence email, which is the exact
 * failure class this module was written to end. Normalising here removes the
 * trap instead of relying on every caller to remember to convert.
 */
function formatExpiry(expiresAt) {
  if (!expiresAt) return '';
  const iso = typeof expiresAt === 'string' ? expiresAt : expiresAt?.toISOString?.();
  if (typeof iso !== 'string' || iso.length < 10) return '';
  return `Valid until: ${iso.slice(0, 10)}`;
}

/**
 * Where the "Activate in 4DA" button points.
 *
 * NOT a custom-scheme URL. Gmail strips custom-scheme hrefs outright, in the
 * browser and in its mobile apps, so that button rendered with no href at all and
 * did nothing when clicked — in the most widely used mail client there is.
 *
 * So we link over https to a bridge page that performs the `fourda://` handoff
 * from a real click on a real web page, which no sanitiser touches. Same pattern
 * as Slack and Zoom desktop handoff. (`fourda`, not `4da`: schemes must start
 * with a letter, so browsers parsed `4da://` as a relative path — see activate.njk.)
 *
 * The key goes in the FRAGMENT: a fragment is never sent to the server, so the
 * licence key stays out of request logs and out of any Referer header. /activate
 * also sets `noAnalytics`, keeping it away from client-side analytics.
 */
function buildActivateUrl(licenseKey) {
  return `https://4da.ai/activate#key=${encodeURIComponent(licenseKey)}`;
}

function buildLicenseEmail(licenseKey, tier, expiresAt, context = 'recovery') {
  const activateUrl = buildActivateUrl(licenseKey);
  const expiryLine = formatExpiry(expiresAt);

  const heading = escapeHtml(SUBJECT[context] || SUBJECT.recovery);
  const content = `              <h1 style="margin: 0 0 6px; font-family: ${FONT_STACK}; font-size: 22px; line-height: 1.3; font-weight: 600; color: ${BRAND.ink};">${heading}</h1>
              <p style="margin: 0 0 22px; font-family: ${FONT_STACK}; font-size: 14px; line-height: 1.5; color: ${BRAND.muted};">Tier: <strong style="color: ${BRAND.ink};">${escapeHtml(tier || 'signal')}</strong>${
                expiryLine ? ` &middot; ${escapeHtml(expiryLine)}` : ''
              }</p>
${keyPanel(escapeHtml(licenseKey))}
              <div style="height: 26px; line-height: 26px; font-size: 0;">&nbsp;</div>
${emailButton(escapeHtml(activateUrl), 'Activate in 4DA')}
              <p style="margin: 22px 0 0; font-family: ${FONT_STACK}; font-size: 13px; line-height: 1.65; color: ${BRAND.muted};">
                Not working? Open 4DA and paste the key into <strong style="color: ${BRAND.ink};">Settings &rarr; License</strong>.
              </p>`;

  const html = renderShell({
    title: heading,
    preheader: escapeHtml(PREHEADER[context] || PREHEADER.recovery),
    badge: context === 'renewal' ? 'Subscription renewed' : 'Signal licence',
    contentHtml: content,
    footerHtml: footerHtml(context),
  });

  const text = [
    SUBJECT[context] || SUBJECT.recovery,
    '',
    `Tier: ${tier || 'signal'}`,
    ...(expiryLine ? [expiryLine] : []),
    '',
    'Licence key:',
    licenseKey,
    '',
    'Activate in 4DA:',
    activateUrl,
    '',
    "If the deep link doesn't work, open 4DA, go to Settings -> License, and",
    'paste the key.',
    footerText(context),
  ].join('\n');

  return { subject: SUBJECT[context] || SUBJECT.recovery, html, text };
}

function buildExpiredEmail(expiredAt) {
  // Same Date-or-string tolerance as formatExpiry, for the same reason.
  const formatted = formatExpiry(expiredAt);
  const when = formatted ? formatted.replace('Valid until: ', '') : 'recently';
  const content = `              <h1 style="margin: 0 0 6px; font-family: ${FONT_STACK}; font-size: 22px; line-height: 1.3; font-weight: 600; color: ${BRAND.ink};">Your 4DA licence has expired</h1>
              <p style="margin: 0 0 24px; font-family: ${FONT_STACK}; font-size: 14px; line-height: 1.6; color: ${BRAND.muted};">The licence on this address expired on <strong style="color: ${BRAND.ink};">${escapeHtml(when)}</strong>, so there is no active key to send.</p>
${emailButton('https://4da.ai/signal', 'Renew Signal')}`;

  const html = renderShell({
    title: 'Your 4DA licence has expired',
    preheader: 'Your 4DA Signal licence has expired, so there is no active key to send.',
    badge: 'Licence expired',
    contentHtml: content,
    footerHtml: footerHtml('recovery'),
  });

  const text = [
    'Your 4DA licence has expired',
    '',
    `The licence on this address expired on ${when}, so there is no active key`,
    'to send.',
    '',
    'Renew: https://4da.ai/signal',
    footerText('recovery'),
  ].join('\n');

  return { subject: 'Your 4DA licence has expired', html, text };
}

function buildRevokedEmail() {
  // Sent instead of the key when the entitlement was refunded or charged back.
  // Mailing the key after the money went back would undermine the refund — the
  // key verifies offline, so a re-delivered copy works until its embedded
  // expiry with nothing to revoke it. The address on file still deserves an
  // honest answer rather than silence (silence reads as a broken recovery form
  // and becomes a support ticket).
  const content = `              <h1 style="margin: 0 0 6px; font-family: ${FONT_STACK}; font-size: 22px; line-height: 1.3; font-weight: 600; color: ${BRAND.ink};">This licence is no longer active</h1>
              <p style="margin: 0 0 24px; font-family: ${FONT_STACK}; font-size: 14px; line-height: 1.6; color: ${BRAND.muted};">The 4DA Signal licence on this address was refunded or its payment was reversed, so there is no active key to send. If you believe this is a mistake, just reply to this email and a person will sort it out.</p>
${emailButton('https://4da.ai/signal', 'Get Signal again')}`;

  const html = renderShell({
    title: 'This licence is no longer active',
    preheader: 'The 4DA Signal licence on this address is no longer active.',
    badge: 'Licence inactive',
    contentHtml: content,
    footerHtml: footerHtml('recovery'),
  });

  const text = [
    'This licence is no longer active',
    '',
    'The 4DA Signal licence on this address was refunded or its payment was',
    'reversed, so there is no active key to send. If you believe this is a',
    'mistake, just reply to this email and a person will sort it out.',
    '',
    'Get Signal again: https://4da.ai/signal',
    footerText('recovery'),
  ].join('\n');

  return { subject: 'Your 4DA licence is no longer active', html, text };
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ---------------------------------------------------------------------------
// Delivery
// ---------------------------------------------------------------------------

async function send(env, to, message) {
  const response = await fetch(RESEND_ENDPOINT, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${env.RESEND_API_KEY}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      from: env.RESEND_FROM_EMAIL,
      to,
      subject: message.subject,
      html: message.html,
      text: message.text,
    }),
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Resend send failed (${response.status}): ${body}`);
  }
}

/**
 * Mail a freshly-issued licence key to the buyer at purchase or renewal.
 *
 * NEVER THROWS, and never fails its caller. This runs off the back of a paid
 * Stripe webhook: throwing would return non-2xx, Stripe would retry, and each
 * retry MINTS ANOTHER KEY (`generateAndStoreLicense` is not idempotent). A
 * failed email must therefore degrade to "the buyer uses the success page or
 * recovery", never to "issue a second entitlement".
 *
 * Unprovisioned mail is logged at error level rather than silently skipped. That
 * is the whole lesson of the recovery path: the 503 there was correct behaviour
 * that nobody could see, so the capability sat dead. A missing key here has to be
 * loud in the logs of every single sale.
 *
 * @param {Record<string,string>} env
 * @param {string} to           address on the Stripe customer/session
 * @param {string} licenseKey
 * @param {string} [tier]
 * @param {string} [expiresAt]  ISO-8601
 * @param {string} [reason]     'purchase' | 'renewal', for logs only
 * @returns {Promise<'sent'|'not_configured'|'error'>} for logging only.
 */
export async function deliverLicenseEmail(env, to, licenseKey, tier, expiresAt, reason = 'purchase') {
  if (!isRecoveryEmailConfigured(env)) {
    console.error(
      `Licence ${reason} email NOT SENT — RESEND_API_KEY/RESEND_FROM_EMAIL unset. ` +
        'The buyer has no emailed copy of their key; the success page is their only ' +
        'delivery. Set both on the Pages project.',
    );
    return 'not_configured';
  }
  if (!to || !licenseKey) {
    console.error(`Licence ${reason} email NOT SENT — missing address or key`);
    return 'error';
  }

  try {
    await send(env, to, buildLicenseEmail(licenseKey, tier, expiresAt, reason));
    console.log(`Licence ${reason} email sent`);
    return 'sent';
  } catch (err) {
    // Swallowed on purpose — see the no-throw contract above.
    console.error(`Licence ${reason} email failed:`, err?.message);
    return 'error';
  }
}

/**
 * Look the address up in Stripe and, if it holds a licence, mail it there.
 *
 * NEVER returns the licence key and NEVER throws — the caller has already sent a
 * constant response to the client by the time this runs (it is scheduled on
 * `waitUntil`), precisely so that neither the response body NOR the response
 * timing can reveal whether the address is a customer.
 *
 * @returns {Promise<'sent'|'revoked_notice'|'expired_notice'|'no_licence'|'error'>} for logging only.
 */
export async function deliverRecoveryEmail(env, stripe, address) {
  try {
    const customers = await stripe.customers.list({ email: address.toLowerCase(), limit: 1 });
    const customer = customers.data[0];
    if (!customer) return 'no_licence';

    const licenseKey = meta(customer.metadata, 'license');
    if (!licenseKey) return 'no_licence';

    // The address we mail is the one STRIPE holds, not the one the caller typed.
    // They are equal here (we looked up by it), but reading it back from the
    // customer record keeps the invariant explicit: we only ever mail an address
    // on file.
    const to = customer.email || address;

    // Refunded / charged back: the key on file still VERIFIES offline until its
    // embedded expiry, so re-mailing it would hand back exactly the access the
    // refund ended. An honest notice instead. A merely CANCELLED subscriber
    // does not take this branch — their paid tail runs to the key's expiry.
    if (isRevoked(customer.metadata)) {
      await send(env, to, buildRevokedEmail());
      return 'revoked_notice';
    }

    const expiresAt = meta(customer.metadata, 'expires_at');
    if (expiresAt && new Date(expiresAt) < new Date()) {
      await send(env, to, buildExpiredEmail(expiresAt));
      return 'expired_notice';
    }

    await send(env, to, buildLicenseEmail(licenseKey, meta(customer.metadata, 'tier'), expiresAt));
    return 'sent';
  } catch (err) {
    // Deliberately swallowed: surfacing this to the caller would leak whether the
    // address exists. Operators see it in the Cloudflare Pages function logs.
    console.error('Recovery email delivery failed:', err?.message);
    return 'error';
  }
}
