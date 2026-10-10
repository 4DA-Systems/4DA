// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import en from '../../locales/en/ui.json';

// Fresh-profile E2E 2026-10-10: Quick Setup and the calibration summary said
// "3 technologies detected" for a scan that detected 14. The 3 were the
// language chips saved to the user's stack (`combined.topics`, then
// `tech_stack`), not the scan's detections. Each label now names what it
// counts. (The first-run overlay's "8 DEPENDENCIES" is fixed at the source:
// `ace_get_scan_summary` counts stored dependencies — see
// dependency_breakdown_counts_stored_packages_once.)
const strings = en as Record<string, string>;

describe('first-run count labels say what they count', () => {
  it('Quick Setup counts the technologies it will add to the stack', () => {
    expect(strings['onboarding.setup.techToAdd']).toMatch(/add to your stack/i);
    expect(strings['onboarding.setup.techDetected']).toBeUndefined();
  });

  it('the setup summary counts the technologies in the stack', () => {
    expect(strings['calibration.onboarding.summaryProjects']).toMatch(/in your stack/i);
    expect(strings['calibration.onboarding.summaryProjects']).not.toMatch(/detected/i);
  });
});
