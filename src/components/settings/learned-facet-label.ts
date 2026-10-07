// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { getSourceFullName } from '../../config/sources';

export interface FacetLabelInput {
  class: string;
  key: string;
  value: string;
}

/** `topic_affinity` -> `topic affinity`; trims and collapses whitespace. */
function humanize(raw: string): string {
  return raw.replace(/_/g, ' ').replace(/\s+/g, ' ').trim();
}

/**
 * Display parts for a learned-preference chip. The facet KEY is what was
 * learned ("rust", "hackernews"); the VALUE is only the signal kind
 * ("engaged", "saved"). Audit 2026-10-07: chips rendered the value alone, so
 * all 23 read "engaged"/"producing" with no name.
 */
export function facetLabel(facet: FacetLabelInput): { name: string; qualifier: string } {
  const key = facet.key.trim();
  const name =
    facet.class === 'source_pref' ? getSourceFullName(key) || humanize(key) : humanize(key);
  const qualifier = humanize(facet.value ?? '');
  if (!name) return { name: qualifier, qualifier: '' };
  return { name, qualifier: qualifier.toLowerCase() === name.toLowerCase() ? '' : qualifier };
}
