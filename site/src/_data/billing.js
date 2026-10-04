// Exposes the subscription-management link to templates as `billing.portalUrl`.
// The URL itself lives in lib/billing.js so the licence emails use the same one.
import { BILLING_PORTAL_URL } from '../../lib/billing.js';

export default {
  portalUrl: BILLING_PORTAL_URL,
};
