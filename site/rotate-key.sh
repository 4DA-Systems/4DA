#!/bin/bash
# Rotate Stripe live secret key in Vercel
# Usage: paste your new sk_live_ key when prompted

echo "Paste your new Stripe secret key (sk_live_...):"
read -s KEY

if [[ ! "$KEY" == sk_live_* ]]; then
  echo "ERROR: Key must start with sk_live_"
  exit 1
fi

echo "Removing old key..."
cd "$(dirname "$0")"
npx vercel env rm STRIPE_SECRET_KEY production --yes 2>/dev/null

echo "Setting new key..."
printf '%s' "$KEY" | npx vercel env add STRIPE_SECRET_KEY production --yes

echo "Deploying..."
npx vercel --prod --yes | tail -3

echo ""
echo "Done! Key rotated and deployed."
