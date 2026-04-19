// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/** Summary of a stack profile available for selection. */
export interface StackProfileSummary {
  id: string;
  name: string;
  core_tech: string[];
  companions: string[];
  competing: string[];
  pain_point_count: number;
  ecosystem_shift_count: number;
}

/** Result from auto-detection of stack profiles via ACE context. */
export interface StackDetection {
  profile_id: string;
  profile_name: string;
  confidence: number;
  matched_tech: string[];
}

/** Summary of the composed (merged) stack for debugging/display. */
export interface ComposedStackSummary {
  active: boolean;
  pain_point_count: number;
  ecosystem_shift_count: number;
  keyword_boost_count: number;
  source_preferences: [string, number][];
  all_tech: string[];
  competing: string[];
}
