/**
 * Paddle → Keygen Webhook Bridge
 *
 * Receives Paddle webhook events and creates/manages Keygen licenses.
 * Deploy as a Vercel serverless function.
 *
 * Flow:
 *   1. Customer buys Signal tier on Paddle checkout
 *   2. Paddle fires subscription.created webhook → this endpoint
 *   3. This endpoint creates a Keygen license with tier metadata
 *   4. Paddle emails the license key to the customer (via fulfillment)
 *   5. Customer pastes key in 4DA → Keygen validates → Signal tier unlocked
 *
 * Environment variables (set in Vercel dashboard):
 *   PADDLE_WEBHOOK_SECRET  — from Paddle dashboard → Notifications → Secret
 *   KEYGEN_ACCOUNT_ID      — "runyourempirehq"
 *   KEYGEN_PRODUCT_TOKEN   — from Keygen dashboard → API Tokens
 *   KEYGEN_POLICY_ID       — the Keygen policy for Signal tier licenses
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

async function createKeygenLicense(
  accountId: string,
  token: string,
  policyId: string,
  paddleSubscriptionId: string,
  paddleCustomerId: string
): Promise<KeygenLicense> {
  const url = `${KEYGEN_BASE}/${accountId}/licenses`;

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
        attributes: {
          metadata: {
            paddleSubscriptionId,
            paddleCustomerId,
            tier: "signal",
          },
        },
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
  // Find the license by Paddle subscription ID metadata
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
    console.error("Missing environment variables");
    return new Response("Server configuration error", { status: 500 });
  }

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

  try {
    switch (event.event_type) {
      // New subscription — create license
      case "subscription.created":
      case "subscription.activated": {
        const license = await createKeygenLicense(
          accountId,
          token,
          policyId,
          subscriptionId,
          customerId
        );
        console.log(
          `License created: ${license.attributes.key} for subscription ${subscriptionId}`
        );
        break;
      }

      // Payment failed or subscription paused — suspend license
      case "subscription.past_due":
      case "subscription.paused": {
        await suspendKeygenLicense(accountId, token, subscriptionId);
        console.log(`License suspended for subscription ${subscriptionId}`);
        break;
      }

      // Subscription resumed — reinstate license
      case "subscription.resumed": {
        await reinstateKeygenLicense(accountId, token, subscriptionId);
        console.log(`License reinstated for subscription ${subscriptionId}`);
        break;
      }

      // Subscription canceled — revoke license
      case "subscription.canceled": {
        await revokeKeygenLicense(accountId, token, subscriptionId);
        console.log(`License revoked for subscription ${subscriptionId}`);
        break;
      }

      default:
        console.log(`Unhandled event type: ${event.event_type}`);
    }
  } catch (error) {
    console.error(`Webhook processing error: ${error}`);
    return new Response("Processing error", { status: 500 });
  }

  return new Response("OK", { status: 200 });
}
