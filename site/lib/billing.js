// Where a Signal subscriber manages their subscription: cancel, update the card,
// download invoices. One definition, read by the site pages (via
// src/_data/billing.js) and by the licence emails.
//
// This is Stripe's no-code customer portal login page (live config
// bpc_1TDrCa04eKAx9AJqKgxHltUa, verified 2026-10-04): the customer enters their
// purchase email and Stripe mails them a one-time sign-in link, so no account
// system of ours is involved and nothing here is a secret.
//
// The portal config has subscription_update (plan switching) DISABLED on
// purpose: renewals re-issue keys from the billing period stored in customer
// metadata (functions/api/license/activate.js), so a monthly -> annual switch
// made in the portal would keep minting 35-day keys. Do not enable switching
// without teaching handleInvoicePaid to read the period from the invoice.
//
// DELIBERATELY DEPENDENCY-FREE, like entitlement.js, so test:scripts can run it.

export const BILLING_PORTAL_URL = 'https://billing.stripe.com/p/login/4gMaEPfCx5An0ZMdDe5Rm00';

// Lifetime keys are issued with a 2099 expiry (generateAndStoreLicense); every
// subscription key expires within ~1 year. Lifetime buyers have nothing to
// cancel, so they are not sent to a subscription portal.
export function isLifetimeExpiry(expiresAt) {
  if (!expiresAt) return false;
  const d = expiresAt instanceof Date ? expiresAt : new Date(expiresAt);
  return !Number.isNaN(d.getTime()) && d.getUTCFullYear() >= 2099;
}
