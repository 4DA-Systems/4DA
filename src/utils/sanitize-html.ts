/**
 * HTML sanitization and URL validation utilities.
 * Used for LLM-generated content, external article snippets, and user-provided URLs.
 *
 * Uses DOMPurify (industry standard) instead of regex — immune to SVG, iframe,
 * HTML5 mutation XSS, and encoding tricks that bypass pattern matching.
 */

import DOMPurify from 'dompurify';

/** Tags considered safe for rendered content. */
const ALLOWED_TAGS = [
  'p',
  'br',
  'a',
  'strong',
  'em',
  'ul',
  'ol',
  'li',
  'code',
  'pre',
  'blockquote',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'span',
  'div',
  'img',
];

/** Attributes considered safe on allowed tags. */
const ALLOWED_ATTR = ['href', 'src', 'alt', 'title', 'class', 'id'];

/**
 * Protocols allowed in href and src attributes.
 * Blocks javascript:, data:, blob:, vbscript:, and everything else.
 */
const ALLOWED_URI_REGEXP = /^(?:https?|mailto):/i;

/**
 * Strip dangerous HTML while preserving safe formatting tags.
 * Removes scripts, event handlers, iframes, embeds, objects, forms,
 * dangerous URI schemes, and any tag/attribute not on the allowlist.
 */
export function sanitizeHtml(html: string): string {
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS,
    ALLOWED_ATTR,
    ALLOWED_URI_REGEXP,
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
  });
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
