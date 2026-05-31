// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/**
 * True when the user has given 4DA nothing to rank against — no detected tech
 * (from a project folder / ACE), no declared interests, and not a single item
 * that scored as relevant. With an empty profile the PASIFA confirmation gate
 * caps every item below the relevance threshold, so relevance is structurally
 * 0 — surfaces should fall back to honest "fresh picks" (recency/quality)
 * rather than claim personalization or show a vanity "0 relevant".
 *
 * Shared by the first-run celebration (FirstRunTransition) and the results
 * sort (use-result-filters) so the two can never disagree about cold-start
 * state.
 */
export function isProfileEmpty(
  detectedTechCount: number,
  interestCount: number,
  hasAnyRelevant: boolean,
): boolean {
  return detectedTechCount === 0 && interestCount === 0 && !hasAnyRelevant;
}
