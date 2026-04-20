// SPDX-License-Identifier: FSL-1.1-Apache-2.0

export type OsvEcosystem = "npm" | "crates.io" | "PyPI" | "Go" | "Maven" | "NuGet" | "RubyGems" | "Packagist";

export interface ResolvedDependency {
  name: string;
  version: string | null;
  ecosystem: OsvEcosystem;
  isDev: boolean;
}

export interface OsvVulnerability {
  id: string;
  summary: string;
  details: string;
  aliases: string[];
  severity: Array<{ type: string; score: string }>;
  affected: Array<{
    package: { name: string; ecosystem: string };
    ranges: Array<{
      type: string;
      events: Array<{ introduced?: string; fixed?: string }>;
    }>;
  }>;
  references: Array<{ type: string; url: string }>;
  published: string;
  modified: string;
}

export interface VulnerabilityEntry {
  package: string;
  currentVersion: string;
  ecosystem: OsvEcosystem;
  isDev: boolean;
  vulnId: string;
  aliases: string[];
  severity: "critical" | "high" | "medium" | "low" | "unknown";
  cvssScore: number | null;
  summary: string;
  fixedVersion: string | null;
  published: string;
  references: string[];
}

export interface VulnerabilityScanResult {
  scannedAt: string;
  projectPath: string;
  ecosystemsScanned: string[];
  totalScanned: number;
  totalVulnerable: number;
  bySeverity: { critical: number; high: number; medium: number; low: number; unknown: number };
  vulnerabilities: VulnerabilityEntry[];
  cleanCount: number;
  scanDurationMs: number;
  cached: boolean;
  offline: boolean;
}

export interface LiveHeadline {
  id: string;
  title: string;
  url: string | null;
  source: "hacker_news";
  points: number;
  comments: number;
  published: string;
  relevanceScore: number;
  relevanceReason: string;
}

export interface LiveIntelligenceStatus {
  enabled: boolean;
  offline: boolean;
  lastOsvRefresh: string | null;
  lastHnRefresh: string | null;
  cachedVulnCount: number;
  cachedHeadlineCount: number;
}
