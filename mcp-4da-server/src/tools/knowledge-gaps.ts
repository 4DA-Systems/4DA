// SPDX-License-Identifier: Apache-2.0
/**
 * knowledge_gaps tool
 *
 * Detect knowledge gaps: direct dependencies with unread, consequential
 * intelligence. The desktop app computes the same concept in
 * `src-tauri/src/knowledge_decay.rs::detect_knowledge_gaps`, and every rule
 * here names the function it mirrors, because two implementations disagreeing
 * is the failure this tool keeps being audited for. Measured 2026-09-11 on the
 * founder machine: five `critical` gaps, four false or inflated, every one
 * contradicting the app.
 */

import type { FourDADatabase } from "../db.js";
import type { LiveIntelligence } from "../live/index.js";
import type { SourceItemBriefRow } from "../types.js";
import { citesDependency, loadCandidates, normalizeGapTitle, type GapCandidate } from "./knowledge-gap-citations.js";
import {
  AdvisoryStore,
  gapExposure,
  installExposure,
  reachingTier,
  rowVerdict,
  type RowVerdict,
} from "./knowledge-gap-exposure.js";
import { InstallResolver, type DeclaringRow, type Install } from "./knowledge-gap-installs.js";
import { gradeGap, isAdvisoryRow, parsePublishedAt, type Exposure, type GapSeverity } from "./knowledge-gap-grading.js";
import { dependencyIsActive, loadActiveScope, loadLinkerIndex, scopeRows, type LinkerIndex } from "./knowledge-gap-scope.js";

// The grading and range rules live in their own modules; re-exported for existing importers.
export {
  advisorySubject,
  gradeGap,
  registryVersionFromTitle,
  type Exposure,
  type GapSeverity,
  type GradableItem,
} from "./knowledge-gap-grading.js";
export { versionInAnyRange, type AdvisoryRangeEvent } from "./knowledge-gap-ranges.js";

export interface KnowledgeGapsParams {
  min_severity?: string;
  limit?: number;
}

export const knowledgeGapsTool = {
  name: "knowledge_gaps",
  description: `Detect knowledge gaps by cross-referencing your project dependencies with source items you haven't engaged with. Identifies things you should know about but might have missed.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      min_severity: {
        type: "string",
        enum: ["critical", "high", "medium", "low"],
        description: "Minimum gap severity to include. Default: medium",
        default: "medium",
      },
      limit: {
        type: "number",
        description: "Maximum gaps to return. Default: 15",
        default: 15,
      },
    },
  },
};

export interface KnowledgeGap {
  dependency: string;
  version: string | null;
  project_path: string;
  language: string;
  missed_items: SourceItemBriefRow[];
  gap_severity: string;
  missed_count: number;
}

const SEVERITY_ORDER: Record<string, number> = { critical: 4, high: 3, medium: 2, low: 1 };
const MAX_MISSED_ITEMS = 5;

/**
 * published_at guard: OSV/CVE backfills ingest decades-old advisories whose
 * created_at (discovery) is days old but whose published_at is ancient. A 2021
 * advisory is not "missed intelligence" in 2026 — unless an install is
 * positively inside it: a still-applying advisory is missed intelligence
 * however old (live: the jsonwebtoken crate advisory, published February,
 * still open against relay/'s 9.3.1 in September). NULL passes (many sources
 * never set it).
 */
const PUBLISHED_WINDOW_MS = 90 * 24 * 60 * 60 * 1000;

/** An advisory row's verdict, or "unresolvable" when the mirror cannot say which advisory it is. */
type Verdict = RowVerdict | "unresolvable";

interface Evidence {
  item: GapCandidate;
  /** Null for a row that is not a registry advisory. */
  verdict: Verdict | null;
}

interface GapContext {
  candidates: GapCandidate[];
  linker: LinkerIndex | null;
  advisories: AdvisoryStore;
  installs: InstallResolver;
  publishedCutoff: number;
}

function passesPublishedCut(e: Evidence, exposure: Exposure, cutoff: number): boolean {
  const publishedAt = parsePublishedAt(e.item.published_at);
  if (publishedAt === null || publishedAt >= cutoff) return true;
  return e.verdict === "reached" || (e.verdict === "unresolvable" && exposure === "exposed");
}

/**
 * Does this evidence keep a security signal alive? A resolved row that reaches
 * an install does; so, conservatively, does a row whose install cannot be read,
 * and a row the mirror cannot resolve while the package is not positively
 * patched (AD-040 rule 3: only exposed installs are findings, and an unknown
 * installed version stays conservatively exposed).
 */
function stillReaches(e: Evidence, exposure: Exposure): boolean {
  return e.verdict === "reached" || e.verdict === "unknown" || (e.verdict === "unresolvable" && exposure !== "safe");
}

function advisoryVerdict(item: GapCandidate, name: string, installs: Install[], store: AdvisoryStore): Verdict {
  const entries = store.forRow(item.source_id, name);
  return entries === null ? "unresolvable" : rowVerdict(entries, installs);
}

function brief(item: GapCandidate): SourceItemBriefRow {
  return {
    id: item.id,
    title: item.title && item.title.length > 120 ? `${item.title.substring(0, 120)}...` : item.title,
    url: item.url,
    source_type: item.source_type,
    created_at: item.created_at,
    relevance_score: item.relevance_score,
  } as SourceItemBriefRow;
}

/** One dependency's gap, judged on every project that declares it, or null when nothing is missed. */
function buildGap(name: string, rows: DeclaringRow[], ctx: GapContext): KnowledgeGap | null {
  const cited = ctx.candidates.filter((item) => citesDependency(item, name, ctx.linker));
  if (cited.length === 0) return null;

  const installs = ctx.installs.installsFor(rows);
  const stored = ctx.advisories.forPackage(name);
  const exposure = gapExposure(installs, stored);

  // An advisory row is judged LIVE on its own advisories against the installs
  // this gap names (`osv::exposure::advisory_row_reaches`, AD-045): a row every
  // install is outside of is not a gap at all. A row the mirror cannot resolve
  // stays in, conservatively. Reposts of one story count once, the newest
  // (`keyword_misses_from`'s normalized-title dedup).
  const evidence: Evidence[] = [];
  const seenTitles = new Set<string>();
  for (const item of cited) {
    const verdict = isAdvisoryRow(item) ? advisoryVerdict(item, name, installs, ctx.advisories) : null;
    if (verdict === "clear") continue;
    const e: Evidence = { item, verdict };
    if (!passesPublishedCut(e, exposure, ctx.publishedCutoff)) continue;
    const titleKey = normalizeGapTitle(item.title || "");
    if (seenTitles.has(titleKey)) continue;
    seenTitles.add(titleKey);
    evidence.push(e);
  }
  if (evidence.length === 0) return null;

  const vulnerable = evidence.some((e) => stillReaches(e, exposure));
  const tier = vulnerable ? reachingTier(stored, installs) : null;
  const versions = installs.map((i) => i.version).filter((v): v is string => v !== null);
  // Each item graded on its own: the gap's grade is the best of them, and the
  // items shown lead with the ones that earn it.
  const graded = evidence
    .map((e, order) => ({
      e,
      order,
      severity: gradeGap([{ ...e.item, cites: true }], name, stillReaches(e, exposure), versions, tier),
    }))
    .sort((a, b) => SEVERITY_ORDER[b.severity] - SEVERITY_ORDER[a.severity] || a.order - b.order);
  const severity: GapSeverity = graded[0].severity;
  const shown = graded.slice(0, MAX_MISSED_ITEMS);

  // AD-044: the row is true for every project it names. When the exposure is
  // what makes this a gap, name the exposed projects with the lead one's own
  // version (`affected_project_paths`); otherwise every declaring project.
  const exposed = vulnerable ? installs.filter((i) => installExposure(i, stored) === "exposed") : [];
  const named = exposed.length > 0 ? exposed : installs;
  const lead = named[0];
  return {
    dependency: name,
    version: lead.version,
    project_path: named.length > 1 ? `${lead.projectPath} (+${named.length - 1} more)` : lead.projectPath,
    language: lead.language,
    missed_items: shown.map(({ e }) => brief(e.item)),
    gap_severity: severity,
    missed_count: shown.length,
  };
}

export function executeKnowledgeGaps(
  db: FourDADatabase,
  params: KnowledgeGapsParams,
  liveIntel?: Pick<LiveIntelligence, "getResolvedDeps" | "isInitialized"> | null,
) {
  // Direct dependencies only — a transitive dep's news is not the user's
  // reading backlog. Dev deps stay in: a vitest or eslint advisory is real.
  // Every direct dependency is scanned: a `LIMIT 100` here once left 43 of
  // 143 unexamined.
  const hasIsDirect = db.hasColumn("project_dependencies", "is_direct");
  const deps = db
    .getRawDb()
    .prepare(
      `SELECT package_name, version, project_path, language FROM project_dependencies ${hasIsDirect ? "WHERE is_direct = 1" : ""}`,
    )
    .all() as DeclaringRow[];

  if (deps.length === 0) {
    return {
      gaps: [],
      summary: "No project dependencies tracked. Add context directories to enable knowledge gap detection.",
    };
  }

  const scope = loadActiveScope(db);
  const ctx: GapContext = {
    candidates: loadCandidates(db),
    linker: loadLinkerIndex(db),
    advisories: new AdvisoryStore(db),
    installs: new InstallResolver(db, liveIntel, scope.rowRoots),
    publishedCutoff: Date.now() - PUBLISHED_WINDOW_MS,
  };

  // One gap per package however many projects declare it; each project's own
  // install is judged inside the gap. Names shorter than 3 characters ("c",
  // "go", "ws") match too much unrelated text to be evidence.
  const byPackage = new Map<string, DeclaringRow[]>();
  for (const row of scopeRows(deps, scope)) {
    if (!row.package_name || row.package_name.length < 3) continue;
    const key = row.package_name.toLowerCase();
    const group = byPackage.get(key);
    if (group) group.push(row);
    else byPackage.set(key, [row]);
  }

  const gaps: KnowledgeGap[] = [];
  for (const rows of byPackage.values()) {
    if (!dependencyIsActive(rows, scope)) continue;
    const gap = buildGap(rows[0].package_name, rows, ctx);
    if (gap) gaps.push(gap);
  }

  const minLevel = SEVERITY_ORDER[params.min_severity || "medium"] || 2;
  const filtered = gaps.filter((g) => (SEVERITY_ORDER[g.gap_severity] || 0) >= minLevel);
  const maxGaps = Math.min(Math.max(1, params.limit || 15), 50);

  return {
    gaps: filtered
      .sort((a, b) => (SEVERITY_ORDER[b.gap_severity] || 0) - (SEVERITY_ORDER[a.gap_severity] || 0))
      .slice(0, maxGaps),
    total_dependencies: deps.length,
    gaps_found: filtered.length,
    gaps_returned: Math.min(filtered.length, maxGaps),
    summary: `${filtered.length} knowledge gaps across ${deps.length} tracked dependencies (showing top ${Math.min(filtered.length, maxGaps)})`,
  };
}
