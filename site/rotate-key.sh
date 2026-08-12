#!/bin/bash
# Rotate the Stripe live secret key on Cloudflare Pages (project: 4da-site).
#
# 4da.ai moved off Vercel to Cloudflare Pages on 2026-07-21. The live API routes
# are Cloudflare Pages Functions in site/functions/, and they read
# STRIPE_SECRET_KEY from the Pages environment -- NOT from Vercel.
#
# `wrangler pages secret put` writes an encrypted secret that subsequent Function
# invocations pick up; no rebuild or redeploy of the static site is required (and
# none is done here -- deploying stays a deliberate, separate step:
# `pnpm run cf:deploy`).
#
# SCOPE: wrangler's `pages secret put` targets the PRODUCTION environment and
# exposes no --env flag (verified against wrangler 4.112.0). To rotate the
# preview-environment secret, use the Cloudflare dashboard:
#   Workers & Pages -> 4da-site -> Settings -> Variables and Secrets -> Preview
#
# Usage: ./rotate-key.sh

set -euo pipefail

cd "$(dirname "$0")"

PROJECT="4da-site"
SECRET_NAME="STRIPE_SECRET_KEY"

echo "Paste your new Stripe secret key (sk_live_...):"
read -r -s KEY
echo ""

if [[ ! "$KEY" == sk_live_* ]]; then
  echo "ERROR: Key must start with sk_live_"
  exit 1
fi

echo "Writing $SECRET_NAME to Cloudflare Pages project '$PROJECT' (production)..."
# `secret put` creates or overwrites, so no separate delete step is needed.
printf '%s' "$KEY" | npx wrangler pages secret put "$SECRET_NAME" --project-name "$PROJECT"

echo ""
echo "Done. Verify the secret is listed with:"
echo "  npx wrangler pages secret list --project-name $PROJECT"
echo ""
echo "The new key takes effect on the next Function invocation. Confirm a live"
echo "checkout succeeds against it BEFORE revoking the old key in the Stripe"
echo "dashboard."
