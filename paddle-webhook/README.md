# 4DA Paddle → Keygen Webhook

Vercel serverless function that receives Paddle subscription events and manages Keygen licenses for the 4DA Signal tier.

## What it does

| Paddle event | Action |
|--------------|--------|
| `subscription.created` / `subscription.activated` | Create Keygen license (idempotent) + email key to customer |
| `subscription.past_due` / `subscription.paused` | Suspend license |
| `subscription.resumed` | Reinstate license |
| `subscription.canceled` | Revoke license |

Idempotency: on retry, existing licenses are reused — no duplicates. Signature verification is HMAC-SHA256 with timing-safe comparison.

## Prerequisites

You need accounts with three services:

1. **Paddle** — payment processor. Sign up at [paddle.com](https://www.paddle.com/).
2. **Keygen** — license delivery and validation. Sign up at [keygen.sh](https://keygen.sh/).
3. **Resend** — transactional email for license delivery. Sign up at [resend.com](https://resend.com/).

Plus:

- **Vercel** — for deploying this function. Free tier is fine.
- A verified sending domain on Resend (e.g. `licenses@4da.ai` — DNS verification takes ~5 minutes).

## Setup checklist

### 1. Paddle product

1. In Paddle, create a product: **4DA Signal**.
2. Add a price: **$12 USD per month** (and optionally `$99 USD per year`).
3. Go to **Developer Tools → Notifications** → create a new destination.
   - URL: `https://<your-vercel-deployment>/api/paddle` (set after first deploy below — you'll update this)
   - Events: check `subscription.created`, `subscription.activated`, `subscription.paused`, `subscription.past_due`, `subscription.resumed`, `subscription.canceled`
4. Copy the **Secret key** (shown once) — this is `PADDLE_WEBHOOK_SECRET`.

**Optional (improves email fallback):** Create an API key at **Developer Tools → Authentication** → this is `PADDLE_API_KEY`. Used as a fallback when `custom_data.email` isn't present on the event.

### 2. Keygen account + policy

1. Create an account at [keygen.sh](https://app.keygen.sh/sign-up). Your account slug (e.g. `runyourempirehq`) becomes `KEYGEN_ACCOUNT_ID`.
2. Create a **Product** (e.g. "4DA Home").
3. Create a **Policy** attached to that product with these settings:
   - **Scheme:** `ED25519_SIGN` (recommended — allows offline validation in 4DA)
   - **Duration:** subscription-aligned (e.g. monthly, plus a grace period)
   - **Max machines:** 3 (matches the Signal tier device limit)
   - **Floating:** enabled (so users can move between machines)
4. Copy the Policy ID — this is `KEYGEN_POLICY_ID`.
5. Generate a **product token** at Keygen → Settings → Tokens. Use a Product-scoped token, not Admin. This is `KEYGEN_PRODUCT_TOKEN`.

### 3. Resend sending domain

1. Create account at [resend.com](https://resend.com).
2. Add sending domain `4da.ai` (or a subdomain like `mail.4da.ai`).
3. Add the SPF + DKIM records Resend gives you to your DNS provider.
4. Wait for verification (usually <5 minutes).
5. Create an API key under **API Keys** with "Sending access" only. This is `RESEND_API_KEY`.
6. Decide your sender identity, e.g. `"4DA <licenses@4da.ai>"`. This is `RESEND_FROM_EMAIL`.

### 4. Paddle Checkout — pass customer email in `custom_data`

On the Paddle Checkout page (or via Paddle.js), pass the customer's email through `customData.email` so the webhook can deliver to them directly:

```js
Paddle.Checkout.open({
  items: [{ priceId: "pri_...", quantity: 1 }],
  customData: { email: customerEmail },
  customer: { email: customerEmail }, // also populates Paddle's customer record
});
```

If you forget, the webhook falls back to Paddle's API (requires `PADDLE_API_KEY`) or logs a warning and leaves the key for manual delivery.

### 5. Deploy to Vercel

```bash
cd paddle-webhook
pnpm install
vercel link   # link to the 4da-home project if not already linked
vercel env add PADDLE_WEBHOOK_SECRET production
vercel env add KEYGEN_ACCOUNT_ID production
vercel env add KEYGEN_PRODUCT_TOKEN production
vercel env add KEYGEN_POLICY_ID production
vercel env add RESEND_API_KEY production
vercel env add RESEND_FROM_EMAIL production
vercel env add PADDLE_API_KEY production   # optional fallback
vercel --prod
```

Vercel prompts for each secret value as you run the `env add` commands.

After the first deploy, take the deployment URL (e.g. `https://4da-home.vercel.app/api/paddle`) and paste it back into Paddle's webhook destination URL.

## Testing

### Smoke test the deployed endpoint

A GET request should return 405 (method not allowed) — that's the correct, secure default:

```bash
curl -i https://your-deployment.vercel.app/api/paddle
# HTTP/2 405
```

### End-to-end: real test purchase

1. In Paddle, switch the webhook destination to **Live mode** (or use Sandbox first).
2. Use Paddle's test card `4242 4242 4242 4242` in Sandbox checkout, or a real card with $0.01 SKU in Live for a true dry run.
3. Complete checkout.
4. Confirm in Vercel logs: you should see `License created: <key>` and `License emailed to <email>` in under 10 seconds.
5. Check the inbox — the email should contain the key and a one-click **Activate in 4DA** button (deep-links to `4da://activate?key=...`).
6. In 4DA: **Settings → License** → paste the key → verify tier flips to **Signal**.

### Paddle retry test

Paddle retries webhook deliveries on 5xx. Verify idempotency:

1. Find the same `subscription.created` event in Paddle's notification log.
2. Click **Resend**.
3. Confirm Vercel logs show `License already exists for <subscription_id>` — no duplicate license created.

## Observability

- **Vercel logs** are the primary observability surface. Each event is logged with `[notification_id]` prefix for correlation.
- For production, consider piping Vercel logs to a log drain (Axiom, Datadog, etc.) — the integration is zero-config via the Vercel dashboard.
- Failed Keygen API calls return HTTP 500 from the webhook, which triggers Paddle's exponential retry (up to 3 days).

## Security notes

- Signature verification uses `timingSafeEqual` — resistant to timing attacks.
- Environment variables are only read at request time; they never hit the client.
- The CORS and security headers in `vercel.json` prevent framing and sniffing.
- Webhook endpoint is POST-only; GET returns 405.
- Keygen tokens are Product-scoped (no account admin access).
- Resend API key has send-only scope.

## Files

| File | Purpose |
|------|---------|
| `api/paddle.ts` | The webhook handler (signature verify → idempotent license ops → email) |
| `vercel.json` | Function config (10s timeout, security headers) |
| `package.json` | Dev dependencies only (`typescript`, `vercel`) — runtime uses Node built-ins + `fetch` |
| `tsconfig.json` | TypeScript strict mode, ES2022 target |
| `README.md` | This file |

## Common issues

**"Invalid signature" 401**
Your `PADDLE_WEBHOOK_SECRET` doesn't match the one Paddle is sending. Copy it again from Paddle → Notifications → your destination → Secret. Note it's shown only once on creation; if you've lost it, rotate it.

**"Keygen license creation failed (401)"**
`KEYGEN_PRODUCT_TOKEN` is wrong or expired. Create a new token in the Keygen dashboard and redeploy the env var.

**"Keygen license creation failed (404)"**
`KEYGEN_POLICY_ID` doesn't exist or belongs to a different product. Double-check the Policy ID.

**No email sent, logs say "Resend not configured"**
`RESEND_API_KEY` or `RESEND_FROM_EMAIL` isn't set. The license was still created — look up the key in Keygen and deliver it manually. Then fix the env vars.

**Resend "403 domain not verified"**
The sending domain in `RESEND_FROM_EMAIL` isn't verified. Confirm SPF + DKIM records in your DNS provider.

**Emails going to spam**
Add a DMARC record (`v=DMARC1; p=none;`) to your domain. Consider warming up the sending domain gradually.

## Rotating secrets

To rotate Paddle webhook secret or any other credential:

```bash
vercel env rm PADDLE_WEBHOOK_SECRET production
vercel env add PADDLE_WEBHOOK_SECRET production
vercel --prod
```

Paddle allows ≥1 active secret at a time, so rotate on a maintenance window.
