// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Source input parser — turns whatever the user pastes into a normalized,
 * ready-to-validate source identifier.
 *
 * Handles:
 *   - Bare domains and paths          → auto-prepend https://
 *   - Full URLs with http/https       → leave as-is
 *   - YouTube channel URLs            → extract channel ID or @handle
 *   - YouTube @handles                → preserve as @handle
 *   - Twitter/X URLs                  → extract handle
 *   - Twitter/X @handles              → preserve as handle (without @)
 *   - GitHub language lists           → split on commas
 *
 * Never throws — always returns a structured hint the caller can act on.
 */

export type SourceKind =
  | 'rss'
  | 'youtube-handle'
  | 'youtube-channel-id'
  | 'twitter-handle'
  | 'github-languages'
  | 'unknown';

export interface ParsedInput {
  kind: SourceKind;
  /** Normalized value ready to pass to the backend */
  value: string;
  /** Human-readable explanation of what we detected */
  detected: string;
  /** Warnings (non-fatal) to show the user */
  warnings: string[];
  /** If the parser auto-corrected the input, show the original */
  original: string;
}

const YT_CHANNEL_ID_RE = /^UC[A-Za-z0-9_-]{20,}$/;
const YT_HANDLE_RE = /^@[A-Za-z0-9_.-]{3,30}$/;
const TWITTER_HANDLE_RE = /^@?[A-Za-z0-9_]{1,15}$/;
const DOMAIN_RE = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+/i;

/**
 * Parse raw user input into a normalized source descriptor.
 * This is the single entry point — callers pick which field to populate
 * based on the returned `kind`.
 */
export function parseSourceInput(raw: string): ParsedInput {
  const trimmed = raw.trim();
  const original = trimmed;
  const warnings: string[] = [];

  if (trimmed === '') {
    return { kind: 'unknown', value: '', detected: '', warnings: [], original };
  }

  // Comma-separated alphanumeric tokens → GitHub languages
  if (/^[a-z0-9+#.,\s-]+$/i.test(trimmed) && trimmed.includes(',')) {
    const langs = trimmed
      .split(',')
      .map((l) => l.trim().toLowerCase())
      .filter((l) => l.length > 0 && l.length < 30);
    if (langs.length > 0) {
      return {
        kind: 'github-languages',
        value: langs.join(','),
        detected: `${langs.length} GitHub language${langs.length === 1 ? '' : 's'}`,
        warnings,
        original,
      };
    }
  }

  // YouTube channel ID (UC-prefix, 22+ chars)
  if (YT_CHANNEL_ID_RE.test(trimmed)) {
    return {
      kind: 'youtube-channel-id',
      value: trimmed,
      detected: 'YouTube channel ID',
      warnings,
      original,
    };
  }

  // Plain @handle — ambiguous between YouTube and Twitter/X.
  // We default to Twitter/X since it's more common as a bare @handle.
  // If it matches YouTube handle rules (longer / allows . and -), we keep
  // it as youtube-handle; shorter ones go to twitter-handle.
  if (trimmed.startsWith('@') && !trimmed.includes('/') && !trimmed.includes(' ')) {
    const withoutAt = trimmed.slice(1);
    // Short @handles → Twitter (twitter limits to 15 chars)
    if (TWITTER_HANDLE_RE.test(trimmed) && withoutAt.length <= 15) {
      return {
        kind: 'twitter-handle',
        value: withoutAt,
        detected: `Twitter/X handle @${withoutAt}`,
        warnings: [
          'Detected as X/Twitter handle. For a YouTube handle, paste the full YouTube URL.',
        ],
        original,
      };
    }
    // Longer @handle → YouTube
    if (YT_HANDLE_RE.test(trimmed)) {
      return {
        kind: 'youtube-handle',
        value: trimmed,
        detected: `YouTube handle ${trimmed}`,
        warnings,
        original,
      };
    }
    return {
      kind: 'twitter-handle',
      value: withoutAt,
      detected: `Treating as @${withoutAt}`,
      warnings,
      original,
    };
  }

  // YouTube URL — extract channel ID or @handle
  const ytMatch = extractYouTubeTarget(trimmed);
  if (ytMatch) {
    return ytMatch;
  }

  // Twitter/X URL — extract handle
  const twMatch = extractTwitterHandle(trimmed);
  if (twMatch) {
    return twMatch;
  }

  // Otherwise: treat as RSS-feed-candidate URL (possibly missing protocol).
  // Auto-prepend https:// if the input looks like a URL/domain.
  let normalized = trimmed;
  if (!/^https?:\/\//i.test(normalized)) {
    // Must look like a domain to auto-correct
    if (DOMAIN_RE.test(normalized.split('/')[0] ?? '')) {
      normalized = `https://${normalized}`;
      warnings.push(`Added "https://" prefix. Original input: ${original}`);
    }
  }

  // Validate it's parseable as a URL
  try {
    // eslint-disable-next-line no-new
    new URL(normalized);
    return {
      kind: 'rss',
      value: normalized,
      detected: detectFeedLikeness(normalized),
      warnings,
      original,
    };
  } catch {
    return {
      kind: 'unknown',
      value: trimmed,
      detected: 'Could not recognize as a URL, handle, or language list',
      warnings: [
        ...warnings,
        'Expected: an RSS/website URL, a YouTube @handle or channel URL, an @handle, or a comma-separated list of languages.',
      ],
      original,
    };
  }
}

function extractYouTubeTarget(raw: string): ParsedInput | null {
  let url: URL;
  try {
    url = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
  } catch {
    return null;
  }
  const host = url.hostname.replace(/^www\./, '').toLowerCase();
  if (host !== 'youtube.com' && host !== 'm.youtube.com' && host !== 'youtu.be') {
    return null;
  }
  const path = url.pathname;
  // /@handle
  const handleMatch = /^\/(@[A-Za-z0-9_.-]{3,30})\b/.exec(path);
  if (handleMatch?.[1]) {
    const handle = handleMatch[1];
    return {
      kind: 'youtube-handle',
      value: handle,
      detected: `YouTube handle ${handle}`,
      warnings: [],
      original: raw,
    };
  }
  // /channel/UCxxxxx
  const idMatch = /^\/channel\/(UC[A-Za-z0-9_-]{20,})\b/.exec(path);
  if (idMatch?.[1]) {
    return {
      kind: 'youtube-channel-id',
      value: idMatch[1],
      detected: 'YouTube channel ID',
      warnings: [],
      original: raw,
    };
  }
  // /c/Name or /user/Name — older formats, cannot resolve without API,
  // pass as a handle-like string with a warning.
  const legacy = /^\/(?:c|user)\/([A-Za-z0-9_-]+)/.exec(path);
  if (legacy) {
    return {
      kind: 'youtube-handle',
      value: `@${legacy[1]}`,
      detected: `YouTube legacy name — will resolve on fetch`,
      warnings: [
        'Legacy /c/ or /user/ URLs may not resolve. If fetch fails, paste the channel @handle instead.',
      ],
      original: raw,
    };
  }
  return null;
}

function extractTwitterHandle(raw: string): ParsedInput | null {
  let url: URL;
  try {
    url = new URL(/^https?:\/\//i.test(raw) ? raw : `https://${raw}`);
  } catch {
    return null;
  }
  const host = url.hostname.replace(/^www\./, '').toLowerCase();
  if (host !== 'twitter.com' && host !== 'x.com' && host !== 'nitter.net') {
    return null;
  }
  const handle = url.pathname.replace(/^\//, '').split('/')[0];
  if (!handle || !TWITTER_HANDLE_RE.test(`@${handle}`)) {
    return null;
  }
  return {
    kind: 'twitter-handle',
    value: handle,
    detected: `X/Twitter handle @${handle}`,
    warnings: [],
    original: raw,
  };
}

/**
 * Quick heuristic for whether a URL path looks feed-like. Used only for the
 * "detected" human-readable label — the actual fetch decides.
 */
function detectFeedLikeness(normalizedUrl: string): string {
  const lower = normalizedUrl.toLowerCase();
  if (lower.includes('/feed') || lower.endsWith('.xml') || lower.endsWith('.rss')
      || lower.includes('/rss') || lower.includes('/atom')) {
    return 'RSS/Atom feed URL';
  }
  return 'Website — will try to autodiscover feed';
}
