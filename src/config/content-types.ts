// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/** Canonical content type badge registry — maps content_dna classifications to display badges. */

export interface ContentTypeMeta {
  /** Short display label */
  label: string;
  /** Tailwind color classes for badge */
  colorClass: string;
}

/** Content types from content_dna.rs classification. "discussion" is omitted (default/fallback). */
const CONTENT_TYPES: Record<string, ContentTypeMeta> = {
  security_advisory: { label: 'Security', colorClass: 'bg-red-500/20 text-red-400' },
  breaking_change: { label: 'Breaking', colorClass: 'bg-amber-500/20 text-amber-400' },
  release_notes: { label: 'Release', colorClass: 'bg-teal-500/20 text-teal-400' },
  deep_dive: { label: 'Deep Dive', colorClass: 'bg-indigo-500/20 text-indigo-400' },
  tutorial: { label: 'Tutorial', colorClass: 'bg-sky-500/20 text-sky-400' },
  show_and_tell: { label: 'Show', colorClass: 'bg-gray-400/20 text-gray-400' },
  question: { label: 'Question', colorClass: 'bg-yellow-500/20 text-yellow-400' },
  hiring: { label: 'Hiring', colorClass: 'bg-gray-500/20 text-gray-500' },
};

/** Get badge metadata for a content type. Returns null for "discussion" or unknown types. */
export function getContentTypeBadge(contentType: string | undefined | null): ContentTypeMeta | null {
  if (!contentType || contentType === 'discussion') return null;
  return CONTENT_TYPES[contentType] ?? null;
}
