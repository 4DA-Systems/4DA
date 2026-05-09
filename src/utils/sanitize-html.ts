// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/**
 * Validate that a URL uses a safe scheme (http or https).
 * Returns true for valid http/https URLs, false for everything else
 * (javascript:, data:, blob:, file:, malformed URLs, etc.)
 */
export function isSafeUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'https:' || parsed.protocol === 'http:';
  } catch {
    return false;
  }
}
