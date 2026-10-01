// SPDX-License-Identifier: Apache-2.0
/**
 * The 4DA app's persisted Upgrade Plan, read for `upgrade_planner` (AD-049).
 *
 * The desktop app writes its ranked plan to `kv_store` under
 * `upgrade_plan_snapshot` after every plan compute (the same envelope
 * `4da plan --json` prints). From schema 4 the envelope carries `steps`: a
 * machine-readable work order keyed by plan item id, built in Rust from the
 * same per-line targets as the item titles. This module only READS it — no
 * range matching, target walking or scope grading is reimplemented here, so
 * the agent gets the app's answer, not a second opinion.
 *
 * Fail-closed like the Rust reader: any schema version other than
 * APP_PLAN_SCHEMA_VERSION is treated as absent and the caller falls back to
 * its standalone heuristic.
 */

import type { FourDADatabase } from "../db.js";

/** The snapshot schema this reader understands (Rust `PLAN_SCHEMA_VERSION`). */
export const APP_PLAN_SCHEMA_VERSION = 4;

const PLAN_KV_KEY = "upgrade_plan_snapshot";

export interface AppPlanSite {
  project: string;
  direct: boolean;
  dev: boolean;
}

export interface AppPlanLine {
  installed: string;
  target: string | null;
  upgrade_type: "patch" | "minor" | "major" | null;
  clears_all_known: boolean;
  sites: AppPlanSite[];
}

export interface AppPlanStep {
  item_id: string;
  ecosystem: string;
  package: string;
  lines: AppPlanLine[];
  mechanism: "manifest_bump" | "lockfile_or_parent_update" | "mixed" | "no_fix";
  advisory_ids: string[];
  verification: string;
}

interface AppPlanItem {
  id: string;
  title: string;
  urgency: string;
}

/** A step with the title and urgency of the plan item it belongs to. */
export interface AppPlanStepView extends AppPlanStep {
  title: string | null;
  urgency: string | null;
}

export interface AppPlanSnapshot {
  schemaVersion: number;
  generatedAt: string;
  expiresAt: string;
  generatorVersion: string | null;
  sourceFreshness: string | null;
  multiVersionCoverage: boolean | null;
  validationDropCount: number | null;
  itemCount: number;
  steps: AppPlanStepView[];
}

export type AppPlanRead =
  | { kind: "plan"; snapshot: AppPlanSnapshot; stale: boolean }
  | { kind: "absent"; reason: string };

/**
 * Read the app's plan from the shared database. Never throws: a missing
 * database, a standalone database without `kv_store`, no snapshot, bad JSON
 * or another schema version all come back as `absent` with the reason.
 */
export function readAppPlan(db: FourDADatabase | null | undefined, now: Date = new Date()): AppPlanRead {
  if (!db) return { kind: "absent", reason: "no 4DA database" };
  let raw: string | undefined;
  try {
    const row = db
      .getRawDb()
      .prepare("SELECT value FROM kv_store WHERE key = ?")
      .get(PLAN_KV_KEY) as { value?: unknown } | undefined;
    raw = typeof row?.value === "string" ? row.value : undefined;
  } catch {
    return { kind: "absent", reason: "the database has no plan store (the 4DA app has not run on it)" };
  }
  if (raw === undefined) {
    return { kind: "absent", reason: "the 4DA app has not computed an upgrade plan yet" };
  }

  let env: Record<string, unknown>;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return { kind: "absent", reason: "the app's plan snapshot is unreadable" };
    }
    env = parsed as Record<string, unknown>;
  } catch {
    return { kind: "absent", reason: "the app's plan snapshot is unreadable" };
  }

  const version = env.schema_version;
  if (version !== APP_PLAN_SCHEMA_VERSION) {
    return {
      kind: "absent",
      reason: `the app's plan snapshot is schema ${String(version)}, this server reads schema ${APP_PLAN_SCHEMA_VERSION}`,
    };
  }
  if (!Array.isArray(env.steps) || !Array.isArray(env.items)) {
    return { kind: "absent", reason: "the app's plan snapshot has no work order" };
  }
  if (typeof env.generated_at !== "string" || typeof env.expires_at !== "string") {
    return { kind: "absent", reason: "the app's plan snapshot has no freshness stamp" };
  }

  const items = new Map<string, AppPlanItem>();
  for (const item of env.items as AppPlanItem[]) {
    if (item && typeof item.id === "string") items.set(item.id, item);
  }
  const steps: AppPlanStepView[] = (env.steps as AppPlanStep[]).map((step) => {
    const item = items.get(step.item_id);
    return { ...step, title: item?.title ?? null, urgency: item?.urgency ?? null };
  });

  const expires = Date.parse(env.expires_at);
  // An unparseable horizon cannot prove freshness: treat it as stale.
  const stale = Number.isNaN(expires) || now.getTime() > expires;

  return {
    kind: "plan",
    stale,
    snapshot: {
      schemaVersion: APP_PLAN_SCHEMA_VERSION,
      generatedAt: env.generated_at,
      expiresAt: env.expires_at,
      generatorVersion: typeof env.generator_version === "string" ? env.generator_version : null,
      sourceFreshness: typeof env.source_freshness === "string" ? env.source_freshness : null,
      multiVersionCoverage: typeof env.multi_version_coverage === "boolean" ? env.multi_version_coverage : null,
      validationDropCount: typeof env.validation_drop_count === "number" ? env.validation_drop_count : null,
      itemCount: items.size,
      steps,
    },
  };
}

const URGENCY_RISK: Record<string, number> = { critical: 3, high: 2, medium: 1, watch: 0 };
const RISK_LEVELS: Record<string, number> = { low: 0, medium: 1, high: 2, critical: 3 };

export interface AppPlanResult {
  generatedAt: string;
  expiresAt: string;
  /** True past `expiresAt`: the security data the plan read may have moved on. */
  stale: boolean;
  staleness: string | null;
  totalSteps: number;
  steps: AppPlanStepView[];
  summary: string;
  snapshot: Omit<AppPlanSnapshot, "steps" | "generatedAt" | "expiresAt">;
  provenance: { mode: "app_plan"; note: string };
}

const APP_PROVENANCE_NOTE =
  "The 4DA desktop app's own cross-project, version-confirmed Upgrade Plan, read from its database " +
  "(the same plan the app shows and `4da plan --json` prints). Each step is keyed by the app's item id.";

/** Shape the app's plan as the tool result; `risk_threshold` / `max_recommendations` narrow it. */
export function formatAppPlan(
  read: Extract<AppPlanRead, { kind: "plan" }>,
  params: { risk_threshold?: string; max_recommendations?: number },
): AppPlanResult {
  const { snapshot, stale } = read;
  const threshold = params.risk_threshold ?? "all";
  let steps = snapshot.steps;
  if (threshold !== "all" && threshold in RISK_LEVELS) {
    steps = steps.filter((s) => (URGENCY_RISK[s.urgency ?? ""] ?? 1) >= RISK_LEVELS[threshold]);
  }
  const selected = params.max_recommendations ? steps.slice(0, params.max_recommendations) : steps;

  const count = (m: AppPlanStep["mechanism"]) => selected.filter((s) => s.mechanism === m).length;
  const majors = selected.filter((s) => s.lines.some((l) => l.upgrade_type === "major")).length;
  const parts = [
    `${selected.length} step${selected.length !== 1 ? "s" : ""} from the 4DA app's plan (generated ${snapshot.generatedAt})`,
  ];
  if (count("manifest_bump") > 0) parts.push(`${count("manifest_bump")} manifest bump(s)`);
  if (count("lockfile_or_parent_update") > 0) {
    parts.push(`${count("lockfile_or_parent_update")} lockfile or parent update(s) (transitive only)`);
  }
  if (count("mixed") > 0) parts.push(`${count("mixed")} mixed (direct in some projects, transitive in others)`);
  if (count("no_fix") > 0) parts.push(`${count("no_fix")} with no fix published`);
  if (majors > 0) parts.push(`${majors} cross a major version`);
  if (selected.length < snapshot.steps.length) parts.push(`${snapshot.steps.length} in the full plan`);
  const staleness = stale
    ? `STALE: past its freshness horizon (${snapshot.expiresAt}). The advisories or installs may have changed since; open the 4DA app to recompute.`
    : null;
  if (staleness) parts.push(staleness);

  return {
    generatedAt: snapshot.generatedAt,
    expiresAt: snapshot.expiresAt,
    stale,
    staleness,
    totalSteps: snapshot.steps.length,
    steps: selected,
    summary: parts.join(". ") + ".",
    snapshot: {
      schemaVersion: snapshot.schemaVersion,
      generatorVersion: snapshot.generatorVersion,
      sourceFreshness: snapshot.sourceFreshness,
      multiVersionCoverage: snapshot.multiVersionCoverage,
      validationDropCount: snapshot.validationDropCount,
      itemCount: snapshot.itemCount,
    },
    provenance: { mode: "app_plan", note: APP_PROVENANCE_NOTE },
  };
}
