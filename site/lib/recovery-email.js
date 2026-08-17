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
import { meta } from './entitlement.js';

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
  <p style="color: #999; font-size: 12px; margin-top: 16px;">
    4DA Systems Pty Ltd &middot; ACN 696 078 841
  </p>`;
const ENTITY_TEXT = '— 4DA Systems Pty Ltd (ACN 696 078 841)';

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
  return `
  <hr style="border: none; border-top: 1px solid #e5e5e5; margin: 32px 0;">
  <p style="color: #666; font-size: 13px;">
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

function buildLicenseEmail(licenseKey, tier, expiresAt, context = 'recovery') {
  const activateUrl = `4da://activate?key=${encodeURIComponent(licenseKey)}`;
  const expiryLine = formatExpiry(expiresAt);

  const html = `<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Your 4DA licence key</title></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 560px; margin: 40px auto; color: #1a1a1a; line-height: 1.55;">
  <h1 style="font-size: 20px; font-weight: 600; margin-bottom: 8px;">${escapeHtml(SUBJECT[context] || SUBJECT.recovery)}</h1>
  <p style="color: #555; margin-top: 0;">Tier: <strong>${escapeHtml(tier || 'signal')}</strong>${
    expiryLine ? ` &middot; ${escapeHtml(expiryLine)}` : ''
  }</p>
  <div style="background: #f5f5f5; border: 1px solid #e5e5e5; border-radius: 8px; padding: 20px; margin: 24px 0; font-family: 'SF Mono', Consolas, monospace; font-size: 13px; word-break: break-all;">
    ${escapeHtml(licenseKey)}
  </div>
  <p style="margin-bottom: 24px;">
    <a href="${escapeHtml(activateUrl)}" style="display: inline-block; background: #D4AF37; color: #0A0A0A; padding: 12px 20px; border-radius: 6px; text-decoration: none; font-weight: 600;">Activate in 4DA</a>
  </p>
  <p style="color: #666; font-size: 13px;">
    If the button doesn't work, open 4DA, go to <strong>Settings &rarr; License</strong>, and paste the key.
  </p>${footerHtml(context)}
</body>
</html>`;

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
  const html = `<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Your 4DA licence has expired</title></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 560px; margin: 40px auto; color: #1a1a1a; line-height: 1.55;">
  <h1 style="font-size: 20px; font-weight: 600; margin-bottom: 8px;">Your 4DA licence has expired</h1>
  <p style="color: #555;">The licence on this address expired on <strong>${escapeHtml(when)}</strong>, so there is no active key to send.</p>
  <p style="margin: 24px 0;">
    <a href="https://4da.ai/signal" style="display: inline-block; background: #D4AF37; color: #0A0A0A; padding: 12px 20px; border-radius: 6px; text-decoration: none; font-weight: 600;">Renew Signal</a>
  </p>${footerHtml('recovery')}
</body>
</html>`;

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
 * @returns {Promise<'sent'|'expired_notice'|'no_licence'|'error'>} for logging only.
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
