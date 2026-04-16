/**
 * Paddle → Keygen Webhook Bridge
 *
 * Receives Paddle webhook events and creates/manages Keygen licenses.
 * Deploy as a Vercel serverless function.
 *
 * Flow:
 *   1. Customer buys Signal tier on Paddle checkout (pass email via custom_data)
 *   2. Paddle fires subscription.created webhook → this endpoint
 *   3. This endpoint checks for existing license (idempotency), creates one if new
 *   4. This endpoint emails the license key + activation deep link to the customer
 *   5. Customer clicks "Activate in 4DA" → 4da://activate?key=... → Signal unlocked
 *
 * Environment variables (set in Vercel dashboard):
 *   PADDLE_WEBHOOK_SECRET  — from Paddle dashboard → Notifications → Secret
 *   KEYGEN_ACCOUNT_ID      — "runyourempirehq"
 *   KEYGEN_PRODUCT_TOKEN   — from Keygen dashboard → API Tokens (admin or product)
 *   KEYGEN_POLICY_ID       — the Keygen policy for Signal tier licenses
 *   RESEND_API_KEY         — from Resend dashboard → API Keys (optional but recommended)
 *   RESEND_FROM_EMAIL      — e.g. "4DA <licenses@4da.ai>" (required if RESEND_API_KEY set)
 *
 * If RESEND_* is not set, the license is still created — you'll need to deliver
 * it via Paddle's custom fulfillment or a manual follow-up. Logs show the key.
 */

import { createHmac, timingSafeEqual } from "crypto";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface PaddleEvent {
  event_type: string;
  data: {
    id: string;
    status: string;
    customer_id: string;
    custom_data?: Record<string, string>;
    items?: Array<{
      price: { product_id: string };
      quantity: number;
    }>;
  };
  notification_id: string;
  occurred_at: string;
}

interface KeygenLicense {
  id: string;
  attributes: {
    key: string;
    status: string;
    metadata: Record<string, string>;
  };
}

interface PaddleCustomer {
  data: {
    id: string;
    email: string;
    name?: string;
  };
}

// ---------------------------------------------------------------------------
// Paddle signature verification
// ---------------------------------------------------------------------------

function verifyPaddleSignature(
  rawBody: string,
  signature: string | null,
  secret: string
): boolean {
  if (!signature) return false;

  // Paddle Billing v2 uses ts=...;h1=... format
  const parts = Object.fromEntries(
    signature.split(";").map((part) => {
      const [key, ...vals] = part.split("=");
      return [key, vals.join("=")];
    })
  );

  const ts = parts["ts"];
  const h1 = parts["h1"];
  if (!ts || !h1) return false;

  const payload = `${ts}:${rawBody}`;
  const computed = createHmac("sha256", secret).update(payload).digest("hex");

  try {
    return timingSafeEqual(Buffer.from(computed), Buffer.from(h1));
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Keygen license management
// ---------------------------------------------------------------------------

const KEYGEN_BASE = "https://api.keygen.sh/v1/accounts";

/** Look up an existing license by Paddle subscription ID. Returns null if not found. */
async function findKeygenLicenseBySubscription(
  accountId: string,
  token: string,
  paddleSubscriptionId: string
): Promise<KeygenLicense | null> {
  const url = `${KEYGEN_BASE}/${accountId}/licenses?metadata[paddleSubscriptionId]=${paddleSubscriptionId}&limit=1`;

  const response = await fetch(url, {
    headers: {
      Accept: "application/vnd.api+json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) return null;

  const json = await response.json();
  const licenses = json.data as KeygenLicense[];
  return licenses.length > 0 ? licenses[0] ?? null : null;
}

async function createKeygenLicense(
  accountId: string,
  token: string,
  policyId: string,
  paddleSubscriptionId: string,
  paddleCustomerId: string,
  customerEmail: string | null
): Promise<KeygenLicense> {
  const url = `${KEYGEN_BASE}/${accountId}/licenses`;

  const metadata: Record<string, string> = {
    paddleSubscriptionId,
    paddleCustomerId,
    tier: "signal",
  };
  if (customerEmail) metadata.customerEmail = customerEmail;

  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/vnd.api+json",
      Accept: "application/vnd.api+json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      data: {
        type: "licenses",
        attributes: { metadata },
        relationships: {
          policy: {
            data: { type: "policies", id: policyId },
          },
        },
      },
    }),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(
      `Keygen license creation failed (${response.status}): ${errorBody}`
    );
  }

  const json = await response.json();
  return json.data as KeygenLicense;
}

async function suspendKeygenLicense(
  accountId: string,
  token: string,
  paddleSubscriptionId: string
): Promise<void> {
  const searchUrl = `${KEYGEN_BASE}/${accountId}/licenses?metadata[paddleSubscriptionId]=${paddleSubscriptionId}`;

  const searchResponse = await fetch(searchUrl, {
    headers: {
      Accept: "application/vnd.api+json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!searchResponse.ok) return;

  const searchJson = await searchResponse.json();
  const licenses = searchJson.data as KeygenLicense[];

  for (const license of licenses) {
    await fetch(
      `${KEYGEN_BASE}/${accountId}/licenses/${license.id}/actions/suspend`,
      {
        method: "POST",
        headers: {
          Accept: "application/vnd.api+json",
          Authorization: `Bearer ${token}`,
        },
      }
    );
  }
}

async function reinstateKeygenLicense(
  accountId: string,
  token: string,
  paddleSubscriptionId: string
): Promise<void> {
  const searchUrl = `${KEYGEN_BASE}/${accountId}/licenses?metadata[paddleSubscriptionId]=${paddleSubscriptionId}`;

  const searchResponse = await fetch(searchUrl, {
    headers: {
      Accept: "application/vnd.api+json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!searchResponse.ok) return;

  const searchJson = await searchResponse.json();
  const licenses = searchJson.data as KeygenLicense[];

  for (const license of licenses) {
    await fetch(
      `${KEYGEN_BASE}/${accountId}/licenses/${license.id}/actions/reinstate`,
      {
        method: "POST",
        headers: {
          Accept: "application/vnd.api+json",
          Authorization: `Bearer ${token}`,
        },
      }
    );
  }
}

async function revokeKeygenLicense(
  accountId: string,
  token: string,
  paddleSubscriptionId: string
): Promise<void> {
  const searchUrl = `${KEYGEN_BASE}/${accountId}/licenses?metadata[paddleSubscriptionId]=${paddleSubscriptionId}`;

  const searchResponse = await fetch(searchUrl, {
    headers: {
      Accept: "application/vnd.api+json",
      Authorization: `Bearer ${token}`,
    },
  });

  if (!searchResponse.ok) return;

  const searchJson = await searchResponse.json();
  const licenses = searchJson.data as KeygenLicense[];

  for (const license of licenses) {
    await fetch(`${KEYGEN_BASE}/${accountId}/licenses/${license.id}`, {
      method: "DELETE",
      headers: {
        Accept: "application/vnd.api+json",
        Authorization: `Bearer ${token}`,
      },
    });
  }
}

// ---------------------------------------------------------------------------
// Customer email resolution
// ---------------------------------------------------------------------------

/** Fetch customer email from Paddle API as fallback when custom_data doesn't have it. */
async function fetchPaddleCustomerEmail(
  paddleApiKey: string | undefined,
  customerId: string
): Promise<string | null> {
  if (!paddleApiKey) return null;
  try {
    const response = await fetch(
      `https://api.paddle.com/customers/${customerId}`,
      { headers: { Authorization: `Bearer ${paddleApiKey}` } }
    );
    if (!response.ok) return null;
    const json = (await response.json()) as PaddleCustomer;
    return json.data?.email ?? null;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Email delivery (Resend)
// ---------------------------------------------------------------------------

function buildLicenseEmailHtml(licenseKey: string): string {
  const activateUrl = `4da://activate?key=${encodeURIComponent(licenseKey)}`;
  return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Your 4DA Signal license</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 560px; margin: 40px auto; color: #1a1a1a; line-height: 1.55;">
  <h1 style="font-size: 20px; font-weight: 600; margin-bottom: 8px;">Welcome to 4DA Signal</h1>
  <p style="color: #555; margin-top: 0;">Your license key is below. Keep it somewhere safe — you'll need it to activate Signal on each device you install 4DA on.</p>

  <div style="background: #f5f5f5; border: 1px solid #e5e5e5; border-radius: 8px; padding: 20px; margin: 24px 0; font-family: 'SF Mono', Consolas, monospace; font-size: 14px; word-break: break-all;">
    ${licenseKey}
  </div>

  <p style="margin-bottom: 24px;">
    <a href="${activateUrl}" style="display: inline-block; background: #D4AF37; color: #0A0A0A; padding: 12px 20px; border-radius: 6px; text-decoration: none; font-weight: 600;">
      Activate in 4DA
    </a>
  </p>

  <p style="color: #666; font-size: 13px;">
    If the button doesn't work, open 4DA, go to <strong>Settings → License</strong>, and paste your key.
  </p>

  <hr style="border: none; border-top: 1px solid #e5e5e5; margin: 32px 0;">

  <p style="color: #666; font-size: 13px;">
    Need help? Reply to this email or visit
    <a href="https://4da.ai/support" style="color: #1a1a1a;">4da.ai/support</a>.
  </p>

  <p style="color: #999; font-size: 12px; margin-top: 24px;">
    4DA Systems Pty Ltd · ACN 696 078 841<br>
    This email was sent because you purchased a Signal subscription.
  </p>
</body>
</html>`;
}

function buildLicenseEmailText(licenseKey: string): string {
  return [
    "Welcome to 4DA Signal",
    "",
    "Your license key:",
    licenseKey,
    "",
    "Activate in 4DA:",
    `4da://activate?key=${encodeURIComponent(licenseKey)}`,
    "",
    "If the deep link doesn't work, open 4DA, go to Settings → License,",
    "and paste your key.",
    "",
    "Need help? Reply to this email or visit https://4da.ai/support",
    "",
    "— 4DA Systems Pty Ltd (ACN 696 078 841)",
  ].join("\n");
}

async function sendLicenseEmail(
  apiKey: string,
  fromEmail: string,
  toEmail: string,
  licenseKey: string
): Promise<void> {
  const response = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      from: fromEmail,
      to: toEmail,
      subject: "Your 4DA Signal license",
      html: buildLicenseEmailHtml(licenseKey),
      text: buildLicenseEmailText(licenseKey),
    }),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    throw new Error(
      `Resend send failed (${response.status}): ${errorBody}`
    );
  }
}

// ---------------------------------------------------------------------------
// Webhook handler
// ---------------------------------------------------------------------------

export default async function handler(req: Request): Promise<Response> {
  if (req.method !== "POST") {
    return new Response("Method not allowed", { status: 405 });
  }

  const secret = process.env.PADDLE_WEBHOOK_SECRET;
  const accountId = process.env.KEYGEN_ACCOUNT_ID;
  const token = process.env.KEYGEN_PRODUCT_TOKEN;
  const policyId = process.env.KEYGEN_POLICY_ID;

  if (!secret || !accountId || !token || !policyId) {
    console.error("Missing required environment variables");
    return new Response("Server configuration error", { status: 500 });
  }

  const resendApiKey = process.env.RESEND_API_KEY;
  const resendFromEmail = process.env.RESEND_FROM_EMAIL;
  const paddleApiKey = process.env.PADDLE_API_KEY;

  const rawBody = await req.text();
  const signature = req.headers.get("paddle-signature");

  if (!verifyPaddleSignature(rawBody, signature, secret)) {
    return new Response("Invalid signature", { status: 401 });
  }

  let event: PaddleEvent;
  try {
    event = JSON.parse(rawBody);
  } catch {
    return new Response("Invalid JSON", { status: 400 });
  }

  const subscriptionId = event.data.id;
  const customerId = event.data.customer_id;
  const notificationId = event.notification_id;

  try {
    switch (event.event_type) {
      // New subscription — create license (idempotent) + email
      case "subscription.created":
      case "subscription.activated": {
        // Idempotency: skip if license already exists for this subscription.
        // Paddle retries on 5xx, so this webhook can be delivered multiple times.
        const existing = await findKeygenLicenseBySubscription(
          accountId,
          token,
          subscriptionId
        );

        let license: KeygenLicense;
        if (existing) {
          license = existing;
          console.log(
            `[${notificationId}] License already exists for ${subscriptionId}: ${license.attributes.key}`
          );
        } else {
          // Resolve customer email: custom_data first, then Paddle API fallback
          let customerEmail = event.data.custom_data?.email ?? null;
          if (!customerEmail) {
            customerEmail = await fetchPaddleCustomerEmail(
              paddleApiKey,
              customerId
            );
          }

          license = await createKeygenLicense(
            accountId,
            token,
            policyId,
            subscriptionId,
            customerId,
            customerEmail
          );
          console.log(
            `[${notificationId}] License created: ${license.attributes.key} for ${subscriptionId}`
          );
        }

        // Email the license key. Non-fatal: if email fails, the license is
        // still valid and the customer can retrieve it from Paddle's support
        // channel or a manual lookup.
        if (resendApiKey && resendFromEmail) {
          const emailTo =
            license.attributes.metadata.customerEmail ||
            event.data.custom_data?.email ||
            (await fetchPaddleCustomerEmail(paddleApiKey, customerId));

          if (emailTo) {
            try {
              await sendLicenseEmail(
                resendApiKey,
                resendFromEmail,
                emailTo,
                license.attributes.key
              );
              console.log(
                `[${notificationId}] License emailed to ${emailTo}`
              );
            } catch (emailError) {
              console.error(
                `[${notificationId}] Email send failed: ${emailError}. License key: ${license.attributes.key}`
              );
            }
          } else {
            console.warn(
              `[${notificationId}] No customer email resolved — skipping send. License key: ${license.attributes.key}`
            );
          }
        } else {
          console.warn(
            `[${notificationId}] Resend not configured — license key not emailed automatically. Key: ${license.attributes.key}`
          );
        }
        break;
      }

      case "subscription.past_due":
      case "subscription.paused": {
        await suspendKeygenLicense(accountId, token, subscriptionId);
        console.log(
          `[${notificationId}] License suspended for ${subscriptionId}`
        );
        break;
      }

      case "subscription.resumed": {
        await reinstateKeygenLicense(accountId, token, subscriptionId);
        console.log(
          `[${notificationId}] License reinstated for ${subscriptionId}`
        );
        break;
      }

      case "subscription.canceled": {
        await revokeKeygenLicense(accountId, token, subscriptionId);
        console.log(
          `[${notificationId}] License revoked for ${subscriptionId}`
        );
        break;
      }

      default:
        console.log(
          `[${notificationId}] Unhandled event type: ${event.event_type}`
        );
    }
  } catch (error) {
    console.error(`[${notificationId}] Processing error: ${error}`);
    // Return 500 so Paddle retries. Idempotency keeps this safe.
    return new Response("Processing error", { status: 500 });
  }

  return new Response("OK", { status: 200 });
}
