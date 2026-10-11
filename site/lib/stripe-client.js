// The one place this site builds a Stripe client (Cloudflare Workers runtime).
// Every function calls createStripe() instead of `new Stripe(...)`, so the API
// version is pinned everywhere (see stripe-api-version.js). A test fails if a
// bare `new Stripe(` appears anywhere else.

import Stripe from 'stripe';
import { STRIPE_API_VERSION } from './stripe-api-version.js';

export { STRIPE_API_VERSION };

export function createStripe(secretKey) {
  return new Stripe(secretKey, {
    apiVersion: STRIPE_API_VERSION,
    httpClient: Stripe.createFetchHttpClient(),
  });
}
