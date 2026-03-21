/**
 * HTML sanitization and URL validation utilities.
 * Used for LLM-generated content, external article snippets, and user-provided URLs.
 */

/**
 * Strip dangerous HTML while preserving safe formatting tags.
 * Removes script tags, event handlers, iframes, and dangerous URI schemes.
 */
export function sanitizeHtml(html: string): string {
  return html
    .replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '')
    .replace(/\son\w+\s*=\s*["'][^"']*["']/gi, '')
    .replace(/\son\w+\s*=\s*[^\s>]*/gi, '')
    .replace(/<iframe\b[^>]*>.*?<\/iframe>/gi, '')
    .replace(/<iframe\b[^>]*>/gi, '')
    .replace(/<object\b[^>]*>.*?<\/object>/gi, '')
    .replace(/<object\b[^>]*>/gi, '')
    .replace(/<embed\b[^>]*>/gi, '')
    .replace(/<form\b[^>]*>.*?<\/form>/gi, '')
    .replace(/<form\b[^>]*>/gi, '')
    .replace(/javascript\s*:/gi, 'blocked:')
    .replace(/data\s*:/gi, 'blocked:');
}

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

/**
 * Return the URL if it's safe, otherwise return undefined.
 * Convenience wrapper for use in href attributes.
 */
export function safeUrl(url: string | null | undefined): string | undefined {
  return isSafeUrl(url) ? url! : undefined;
}
