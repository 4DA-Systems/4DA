// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// THE canonical frontend URL identity for deduplication — a faithful port of
// the backend's `scoring::dedup::normalize_result_url` (src-tauri/src/scoring/
// dedup.rs), which is the single source of truth for these semantics. Keep the
// two in lockstep: strip protocol variance, `www.`, trailing slash, fragment,
// and tracking parameters — but PRESERVE content-bearing query parameters
// (YouTube's `?v=` IS the identity of the page), sorted by key so parameter
// order cannot defeat dedup. Keys are lowercased; values keep their case
// (`v=dQw4w9WgXcQ` is a different video from `v=dqw4w9wgxcq`).

/** Query parameters that identify the CAMPAIGN or the referrer, not the CONTENT. */
function isTrackingParam(key: string): boolean {
  return (
    key.startsWith('utm_') ||
    key === 'ref' ||
    key === 'ref_src' ||
    key === 'fbclid' ||
    key === 'gclid' ||
    key === 'si' ||
    key === 'igshid'
  );
}

/**
 * Normalize a URL to its dedup identity. Returns `null` for missing/empty
 * URLs so callers can skip URL-keyed dedup for items without one.
 */
export function normalizeUrlForDedup(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  if (!trimmed) return null;

  const withoutFragment = trimmed.split('#')[0]!;
  const queryIndex = withoutFragment.indexOf('?');
  const pathPart = queryIndex >= 0 ? withoutFragment.slice(0, queryIndex) : withoutFragment;
  const queryPart = queryIndex >= 0 ? withoutFragment.slice(queryIndex + 1) : null;

  const base = pathPart
    .replace('http://', 'https://')
    .replace('://www.', '://')
    .replace(/\/+$/, '')
    .toLowerCase();

  if (queryPart === null) return base;

  const params = queryPart
    .split('&')
    .filter((pair) => pair.length > 0)
    .map((pair) => {
      const eq = pair.indexOf('=');
      return eq >= 0
        ? { key: pair.slice(0, eq).toLowerCase(), value: pair.slice(eq + 1) }
        : { key: pair.toLowerCase(), value: null };
    })
    .filter((p) => !isTrackingParam(p.key));

  if (params.length === 0) return base;

  params.sort((a, b) =>
    a.key === b.key
      ? (a.value ?? '') < (b.value ?? '') ? -1 : 1
      : a.key < b.key ? -1 : 1,
  );
  const normalizedQuery = params
    .map((p) => (p.value !== null ? `${p.key}=${p.value}` : p.key))
    .join('&');
  return `${base}?${normalizedQuery}`;
}
