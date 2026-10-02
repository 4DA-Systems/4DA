// SPDX-License-Identifier: Apache-2.0
/**
 * upgrade_planner reads the 4DA app's plan (AD-049): one brain.
 *
 * Fixture databases hold the `upgrade_plan_snapshot` envelope the Rust app
 * writes (`evidence::plan_snapshot::persist_upgrade_plan`, schema 4). The tool
 * returns that plan's work order when it is current, returns it flagged stale
 * past `expires_at`, and falls back to the standalone heuristic — saying why —
 * when the snapshot is missing, from another schema, or the store is absent.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeUpgradePlanner } from "../tools/upgrade-planner.js";
import { readAppPlan, APP_PLAN_SCHEMA_VERSION, type AppPlanResult } from "../tools/app-plan.js";

let root: string;
const opened: FourDADatabase[] = [];

const UNDICI_ID = "upgrade-plan:npm:undici";
const BRACE_ID = "upgrade-plan:npm:brace-expansion";
const RSA_ID = "upgrade-plan:crates.io:rsa";

function envelope(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  const hour = 3_600_000;
  return {
    schema_version: APP_PLAN_SCHEMA_VERSION,
    generated_at: new Date(Date.now() - hour).toISOString(),
    expires_at: new Date(Date.now() + 23 * hour).toISOString(),
    generator_version: "1.0.3",
    entitlement_scope_at_generation: "signal",
    multi_version_coverage: true,
    dependency_inventory_hash: "abc",
    validation_drop_count: 0,
    source_freshness: "2026-10-01 00:00:00",
    engine_run_id: null,
    item_count: 3,
    items: [
      { id: UNDICI_ID, title: "Upgrade undici to >= 6.28.1 (major) — clears 3 advisories across 1 project", urgency: "high" },
      { id: BRACE_ID, title: "Upgrade brace-expansion to >= 1.1.21 — clears 1 advisory across 2 projects", urgency: "medium" },
      { id: RSA_ID, title: "No fix published for rsa — 1 advisory across 1 project", urgency: "medium" },
    ],
    steps: [
      {
        item_id: UNDICI_ID,
        ecosystem: "npm",
        package: "undici",
        lines: [
          {
            installed: "5.28.4",
            target: "6.28.1",
            upgrade_type: "major",
            clears_all_known: true,
            sites: [{ project: "c:/users/dev/navcal", direct: false, dev: false }],
          },
        ],
        mechanism: "lockfile_or_parent_update",
        advisory_ids: ["GHSA-3wwx-pv8p-q78v", "GHSA-8xcm-r25x-g524", "GHSA-cxrh-j4jr-qwg3"],
        verification: "re-run 4da plan --json",
      },
      {
        item_id: BRACE_ID,
        ecosystem: "npm",
        package: "brace-expansion",
        lines: [
          {
            installed: "1.1.12",
            target: "1.1.21",
            upgrade_type: "patch",
            clears_all_known: true,
            sites: [
              { project: "c:/users/dev/navcal", direct: false, dev: true },
              { project: "d:/web", direct: true, dev: false },
            ],
          },
        ],
        mechanism: "mixed",
        advisory_ids: ["GHSA-q2hr-2g5m-vwhr"],
        verification: "re-run 4da plan --json",
      },
      {
        item_id: RSA_ID,
        ecosystem: "crates.io",
        package: "rsa",
        lines: [
          {
            installed: "0.9.8",
            target: null,
            upgrade_type: null,
            clears_all_known: false,
            sites: [{ project: "d:/relay", direct: true, dev: false }],
          },
        ],
        mechanism: "no_fix",
        advisory_ids: ["RUSTSEC-2023-0071"],
        verification: "re-run 4da plan --json",
      },
    ],
    ...overrides,
  };
}

/** A database file as the app leaves it: a `kv_store` with (optionally) a plan. */
function dbWith(name: string, snapshot: string | null, withStore = true): FourDADatabase {
  const file = path.join(root, `${name}.db`);
  const raw = new Database(file);
  if (withStore) {
    raw.exec("CREATE TABLE kv_store (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    if (snapshot !== null) {
      raw.prepare("INSERT INTO kv_store (key, value) VALUES (?, ?)").run("upgrade_plan_snapshot", snapshot);
    }
  } else {
    raw.exec("CREATE TABLE unrelated (id INTEGER)");
  }
  raw.close();
  const db = new FourDADatabase(file);
  opened.push(db);
  return db;
}

beforeAll(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "4da-app-plan-"));
});

afterAll(() => {
  for (const db of opened) db.close();
  fs.rmSync(root, { recursive: true, force: true });
});

describe("upgrade_planner returns the app's plan when there is one", () => {
  it("a fresh snapshot comes back as the app's work order, targets untouched", async () => {
    const db = dbWith("fresh", JSON.stringify(envelope()));
    const result = (await executeUpgradePlanner(db, {}, null)) as AppPlanResult;

    expect(result.provenance.mode).toBe("app_plan");
    expect(result.stale).toBe(false);
    expect(result.staleness).toBeNull();
    expect(result.totalSteps).toBe(3);
    expect(result.steps.map((s) => s.item_id)).toEqual([UNDICI_ID, BRACE_ID, RSA_ID]);

    const undici = result.steps[0];
    expect(undici.lines[0]).toMatchObject({ installed: "5.28.4", target: "6.28.1", upgrade_type: "major" });
    expect(undici.mechanism).toBe("lockfile_or_parent_update");
    // The item's own title and urgency travel with the step.
    expect(undici.title).toContain(">= 6.28.1");
    expect(undici.urgency).toBe("high");

    expect(result.summary).toContain("1 lockfile or parent update(s)");
    expect(result.summary).toContain("1 mixed");
    expect(result.summary).toContain("1 with no fix published");
    expect(result.summary).toContain("1 cross a major version");
    expect(result.snapshot).toMatchObject({ schemaVersion: 4, itemCount: 3, multiVersionCoverage: true });
  });

  it("an expired snapshot is still the app's plan, flagged stale", async () => {
    const db = dbWith(
      "expired",
      JSON.stringify(
        envelope({
          generated_at: "2026-09-01T00:00:00Z",
          expires_at: "2026-09-02T00:00:00Z",
        }),
      ),
    );
    const result = (await executeUpgradePlanner(db, {}, null)) as AppPlanResult;
    expect(result.provenance.mode).toBe("app_plan");
    expect(result.stale).toBe(true);
    expect(result.staleness).toContain("STALE");
    expect(result.summary).toContain("STALE");
    expect(result.steps).toHaveLength(3);
  });

  it("an empty plan is the app's answer (evaluated, nothing to do), not a reason to guess", async () => {
    const db = dbWith("empty", JSON.stringify(envelope({ items: [], steps: [], item_count: 0 })));
    const result = (await executeUpgradePlanner(db, {}, null)) as AppPlanResult;
    expect(result.provenance.mode).toBe("app_plan");
    expect(result.steps).toEqual([]);
    expect(result.summary).toMatch(/^0 steps from the 4DA app's plan/);
  });

  it("risk_threshold filters by the app's urgency and max_recommendations limits the steps", async () => {
    const db = dbWith("filtered", JSON.stringify(envelope()));
    const high = (await executeUpgradePlanner(db, { risk_threshold: "high" }, null)) as AppPlanResult;
    expect(high.steps.map((s) => s.item_id)).toEqual([UNDICI_ID]);
    const two = (await executeUpgradePlanner(db, { max_recommendations: 2 }, null)) as AppPlanResult;
    expect(two.steps).toHaveLength(2);
    expect(two.totalSteps).toBe(3);
    expect(two.summary).toContain("3 in the full plan");
  });

  it("returns 20 steps by default, like the standalone plan, and says how to get the rest", async () => {
    const env = envelope() as { steps: unknown[] };
    const many = { ...env, steps: Array.from({ length: 25 }, (_, i) => ({ ...(env.steps[0] as object), item_id: `upgrade-plan:npm:pkg${i}`, package: `pkg${i}` })) };
    const db = dbWith("many-steps", JSON.stringify(many));
    const result = (await executeUpgradePlanner(db, {}, null)) as AppPlanResult;
    expect(result.steps).toHaveLength(20);
    expect(result.totalSteps).toBe(25);
    expect(result.summary).toContain("25 in the full plan (raise max_recommendations for the rest)");
  });

  it("package narrows the app's plan to that package, as it does the standalone plan", async () => {
    const db = dbWith("one-package", JSON.stringify(envelope()));
    const one = (await executeUpgradePlanner(db, { package: "UNDICI" }, null)) as AppPlanResult;
    expect(one.steps.map((s) => s.item_id)).toEqual([UNDICI_ID]);
    expect(one.totalSteps).toBe(3);
    const none = (await executeUpgradePlanner(db, { package: "not-in-plan" }, null)) as AppPlanResult;
    expect(none.steps).toEqual([]);
  });
});

describe("upgrade_planner falls back to the standalone heuristic, and says why", () => {
  async function fallback(db: FourDADatabase | null) {
    const result = await executeUpgradePlanner(db as FourDADatabase, {}, null);
    expect(result.provenance.mode).toBe("standalone_heuristic");
    return result as { appPlanUnavailable?: string };
  }

  it("no snapshot computed yet", async () => {
    const result = await fallback(dbWith("missing", null));
    expect(result.appPlanUnavailable).toContain("has not computed an upgrade plan");
  });

  it("an older schema (v3, no work order) is never read as a v4 plan", async () => {
    const v3 = envelope({ schema_version: 3 });
    delete v3.steps;
    const result = await fallback(dbWith("v3", JSON.stringify(v3)));
    expect(result.appPlanUnavailable).toContain("schema 3");
  });

  it("a newer schema is not trusted either", async () => {
    const result = await fallback(dbWith("v5", JSON.stringify(envelope({ schema_version: 5 }))));
    expect(result.appPlanUnavailable).toContain("schema 5");
  });

  it("a database without the plan store (standalone MCP database)", async () => {
    const result = await fallback(dbWith("nostore", null, false));
    expect(result.appPlanUnavailable).toContain("no plan store");
  });

  it("garbage JSON and no database at all", async () => {
    expect((await fallback(dbWith("garbage", "not json"))).appPlanUnavailable).toContain("unreadable");
    expect((await fallback(null)).appPlanUnavailable).toContain("no 4DA database");
  });
});

describe("readAppPlan", () => {
  it("an unparseable expires_at cannot prove freshness: stale", () => {
    const read = readAppPlan(dbWith("badexpiry", JSON.stringify(envelope({ expires_at: "soon" }))));
    expect(read.kind).toBe("plan");
    if (read.kind === "plan") expect(read.stale).toBe(true);
  });

  it("a step whose item is missing keeps its fields, with no title", () => {
    const env = envelope();
    (env.items as unknown[]).splice(0, 1);
    const read = readAppPlan(dbWith("orphan", JSON.stringify(env)));
    expect(read.kind).toBe("plan");
    if (read.kind === "plan") {
      expect(read.snapshot.steps[0].title).toBeNull();
      expect(read.snapshot.steps[0].lines[0].target).toBe("6.28.1");
    }
  });
});
