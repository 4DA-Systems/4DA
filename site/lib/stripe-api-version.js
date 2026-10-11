// The Stripe API version every request from this site uses. PINNED on purpose.
//
// Unpinned, the client sends whatever its stripe-node release defaults to, so an
// SDK major silently moves every payment call to a new API version: stripe-node
// 20.3.1 sends 2026-01-28.clover, 22.6.2 sends 2026-08-26.dahlia (Dependabot
// #856, 2026-10-11). Change this value deliberately — after checking that
// version's changelog against the Stripe fields these functions read — never as a
// side effect of an SDK bump.
//
// Webhook payloads are shaped by the endpoint's API version set in the Stripe
// dashboard, not by this value.
//
// Its own module, with no imports, so the Node test runner can read it without
// the site's dependency tree (see lib/entitlement.js).
export const STRIPE_API_VERSION = '2026-01-28.clover';
