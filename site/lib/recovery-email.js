// Licence-recovery email delivery for Cloudflare Pages Functions (Resend).
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

const RESEND_ENDPOINT = 'https://api.resend.com/emails';

/**
 * True when outbound recovery mail is provisioned. When false the caller MUST
 * fail the email path honestly (tell the user to contact support) rather than
 * fall back to returning the key — returning the key is the vulnerability.
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

const FOOTER_HTML = `
  <hr style="border: none; border-top: 1px solid #e5e5e5; margin: 32px 0;">
  <p style="color: #666; font-size: 13px;">
    You are receiving this because someone asked 4DA to recover the licence for this
    address. If that wasn't you, no action is needed — nothing about your account changed.
  </p>
  <p style="color: #999; font-size: 12px; margin-top: 16px;">
    4DA Systems Pty Ltd &middot; ACN 696 078 841
  </p>`;

const FOOTER_TEXT = [
  '',
  'You are receiving this because someone asked 4DA to recover the licence for',
  "this address. If that wasn't you, no action is needed — nothing about your",
  'account changed.',
  '',
  '— 4DA Systems Pty Ltd (ACN 696 078 841)',
].join('\n');

function buildLicenseEmail(licenseKey, tier, expiresAt) {
  const activateUrl = `4da://activate?key=${encodeURIComponent(licenseKey)}`;
  const expiryLine = expiresAt ? `Valid until: ${expiresAt.slice(0, 10)}` : '';

  const html = `<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Your 4DA licence key</title></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 560px; margin: 40px auto; color: #1a1a1a; line-height: 1.55;">
  <h1 style="font-size: 20px; font-weight: 600; margin-bottom: 8px;">Your 4DA licence key</h1>
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
  </p>${FOOTER_HTML}
</body>
</html>`;

  const text = [
    'Your 4DA licence key',
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
    FOOTER_TEXT,
  ].join('\n');

  return { subject: 'Your 4DA licence key', html, text };
}

function buildExpiredEmail(expiredAt) {
  const when = expiredAt ? expiredAt.slice(0, 10) : 'recently';
  const html = `<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>Your 4DA licence has expired</title></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 560px; margin: 40px auto; color: #1a1a1a; line-height: 1.55;">
  <h1 style="font-size: 20px; font-weight: 600; margin-bottom: 8px;">Your 4DA licence has expired</h1>
  <p style="color: #555;">The licence on this address expired on <strong>${escapeHtml(when)}</strong>, so there is no active key to send.</p>
  <p style="margin: 24px 0;">
    <a href="https://4da.ai/signal" style="display: inline-block; background: #D4AF37; color: #0A0A0A; padding: 12px 20px; border-radius: 6px; text-decoration: none; font-weight: 600;">Renew Signal</a>
  </p>${FOOTER_HTML}
</body>
</html>`;

  const text = [
    'Your 4DA licence has expired',
    '',
    `The licence on this address expired on ${when}, so there is no active key`,
    'to send.',
    '',
    'Renew: https://4da.ai/signal',
    FOOTER_TEXT,
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

    const licenseKey = customer.metadata?.streets_license;
    if (!licenseKey) return 'no_licence';

    // The address we mail is the one STRIPE holds, not the one the caller typed.
    // They are equal here (we looked up by it), but reading it back from the
    // customer record keeps the invariant explicit: we only ever mail an address
    // on file.
    const to = customer.email || address;

    const expiresAt = customer.metadata?.streets_expires_at;
    if (expiresAt && new Date(expiresAt) < new Date()) {
      await send(env, to, buildExpiredEmail(expiresAt));
      return 'expired_notice';
    }

    await send(env, to, buildLicenseEmail(licenseKey, customer.metadata?.streets_tier, expiresAt));
    return 'sent';
  } catch (err) {
    // Deliberately swallowed: surfacing this to the caller would leak whether the
    // address exists. Operators see it in the Cloudflare Pages function logs.
    console.error('Recovery email delivery failed:', err?.message);
    return 'error';
  }
}
