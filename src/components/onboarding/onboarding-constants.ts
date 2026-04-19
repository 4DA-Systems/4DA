// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export const fallbackSuggestions = [
  'Machine Learning', 'Rust', 'TypeScript', 'Web Development',
  'DevOps', 'Security', 'Startups', 'Open Source', 'AI/LLM',
  'Mobile Development', 'Cloud Infrastructure', 'Data Engineering',
];

export const SECTION_KEY = '4da-onboarding-step';

export interface SectionState {
  aiOpen?: boolean;
  projectsOpen?: boolean;
  stacksOpen?: boolean;
  interestsOpen?: boolean;
  localeOpen?: boolean;
  experienceOpen?: boolean;
}

export function getPersistedSections(): SectionState {
  try {
    const stored = localStorage.getItem(SECTION_KEY);
    if (stored) return JSON.parse(stored) as SectionState;
  } catch { /* localStorage unavailable or corrupted */ }
  return {};
}
