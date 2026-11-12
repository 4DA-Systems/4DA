// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * What the onboarding project scan found (audit 2026-11-11).
 *
 * The wizard used to discard the discovery result, swallow its errors and
 * finish onboarding regardless: a scan that found nothing said nothing, and
 * the user met an empty app. The outcome is now shown before onboarding
 * ends, and a scan that found nothing keeps the user on the step.
 */
import type { DiscoveryResult } from '../../lib/discovery';
import type { ScanSummary } from '../first-run/utils';

export interface EcosystemCount {
  ecosystem: 'npm' | 'rust' | 'python' | 'other';
  count: number;
}

export type ScanOutcome =
  | { kind: 'found'; projects: number; dependencies: number; ecosystems: EcosystemCount[] }
  | { kind: 'none'; directories: number }
  | { kind: 'error'; message: string };

const ORDER: EcosystemCount['ecosystem'][] = ['npm', 'rust', 'python', 'other'];

export function scanOutcome(result: DiscoveryResult, summary: ScanSummary | null): ScanOutcome {
  if (result.status === 'no_directories') {
    return { kind: 'none', directories: 0 };
  }
  if (!result.success) {
    return { kind: 'error', message: result.message ?? '' };
  }
  const dependencies = summary?.total_dependencies ?? 0;
  const projects = Math.max(result.projects_found, summary?.projects_scanned ?? 0);
  if (projects === 0 && dependencies === 0) {
    return { kind: 'none', directories: result.directories_found };
  }
  const by = summary?.dependencies_by_ecosystem;
  const ecosystems = by
    ? ORDER.map(ecosystem => ({ ecosystem, count: by[ecosystem] ?? 0 })).filter(e => e.count > 0)
    : [];
  return { kind: 'found', projects, dependencies, ecosystems };
}
