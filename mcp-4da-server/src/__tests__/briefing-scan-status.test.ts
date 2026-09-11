// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for the 2026-09-07 cold-start briefing defect.
 *
 * Verified live: the first `what_should_i_know` after the MCP process started,
 * for "Upgrade jsonwebtoken in the relay service and review auth token
 * verification", returned `advisories: []` and `safe_to_delegate`. The
 * identical call seven minutes later returned 11 advisories and `human_only`.
 *
 * Root cause: full-database mode initialised the live layer without ever
 * starting a vulnerability scan, so `lastVulnScan` stayed null until some tool
 * called vulnerability_scan; the briefing read that empty cache inside a
 * try/catch and silently got nothing. Secondary: the feed pass only reached
 * 72 hours back, and the advisory row was 72.8 hours old.
 *
 * Contract pinned here: the briefing awaits the scan (bounded), records
 * `scan_status`, and never answers "safe" without a ready scan.
 */
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import Database from "better-sqlite3";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { FourDADatabase } from "../db.js";
import { LiveIntelligence } from "../live/index.js";
import { executeWhatShouldIKnow, type BriefingLiveIntel } from "../tools/what-should-i-know.js";
import { clusterVulnerabilities, executeGetActionableSignals } from "../tools/get-actionable-signals.js";
import { normalizeStoredPriority } from "../tools/signal-classifier.js";
import type { ResolvedDependency, VulnerabilityEntry, VulnerabilityScanResult } from "../live/types.js";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SCHEMA = `
  CREATE TABLE source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    url TEXT,
    title TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen TEXT NOT NULL DEFAULT (datetime('now')),
    relevance_score REAL,
    content_type TEXT,
    signal_type TEXT,
    signal_priority TEXT,
    UNIQUE(source_type, source_id)
  );
  CREATE TABLE user_identity (id INTEGER PRIMARY KEY CHECK (id = 1), role TEXT);
  CREATE TABLE tech_stack (id INTEGER PRIMARY KEY AUTOINCREMENT, technology TEXT NOT NULL UNIQUE);
  CREATE TABLE domains (id INTEGER PRIMARY KEY AUTOINCREMENT, domain TEXT NOT NULL UNIQUE);
  CREATE TABLE explicit_interests (id INTEGER PRIMARY KEY AUTOINCREMENT, topic TEXT NOT NULL UNIQUE, weight REAL DEFAULT 1.0, source TEXT DEFAULT 'explicit');
  CREATE TABLE exclusions (id INTEGER PRIMARY KEY AUTOINCREMENT, topic TEXT NOT NULL UNIQUE);
  CREATE TABLE detected_tech (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, category TEXT NOT NULL, confidence REAL DEFAULT 0.5, source TEXT NOT NULL);
  CREATE TABLE active_topics (id INTEGER PRIMARY KEY AUTOINCREMENT, topic TEXT NOT NULL UNIQUE, weight REAL DEFAULT 0.5, confidence REAL DEFAULT 0.5, source TEXT NOT NULL, last_seen TEXT DEFAULT (datetime('now')));
  CREATE TABLE interactions (id INTEGER PRIMARY KEY AUTOINCREMENT, item_id INTEGER, action_type TEXT, item_source TEXT, signal_strength REAL DEFAULT 0.5, timestamp TEXT DEFAULT (datetime('now')));
  CREATE TABLE developer_decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT, decision_type TEXT NOT NULL, subject TEXT NOT NULL, decision TEXT NOT NULL,
    rationale TEXT, alternatives_rejected TEXT DEFAULT '[]', context_tags TEXT DEFAULT '[]', confidence REAL NOT NULL DEFAULT 0.8,
    status TEXT NOT NULL DEFAULT 'active', superseded_by INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
  CREATE TABLE agent_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, agent_type TEXT NOT NULL, memory_type TEXT NOT NULL,
    subject TEXT NOT NULL, content TEXT NOT NULL, context_tags TEXT DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')), expires_at TEXT, promoted_to_decision_id INTEGER
  );
`;

function createTestDatabase(): FourDADatabase {
  const rawDb = new Database(":memory:");
  rawDb.exec(SCHEMA);
  rawDb.prepare("INSERT INTO user_identity (id, role) VALUES (1, 'Senior Developer')").run();
  rawDb.prepare("INSERT INTO tech_stack (technology) VALUES ('rust')").run();
  const instance = Object.create(FourDADatabase.prototype) as FourDADatabase;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (instance as any).db = rawDb;
  return instance;
}

function sqlDate(msAgo: number): string {
  return new Date(Date.now() - msAgo).toISOString().replace("T", " ").slice(0, 19);
}
const HOURS = 60 * 60 * 1000;

let seq = 0;
function insertItem(
  db: FourDADatabase,
  over: {
    title: string;
    hoursAgo: number;
    relevance?: number;
    signal_type?: string | null;
    signal_priority?: string | null;
    source_type?: string;
  },
): number {
  seq += 1;
  const result = db
    .getRawDb()
    .prepare(
      `INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, created_at, last_seen, relevance_score, signal_type, signal_priority)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      over.source_type ?? "osv",
      `item-${seq}`,
      "https://example.com/" + seq,
      over.title,
      over.title,
      `hash-${seq}`,
      sqlDate(over.hoursAgo * HOURS),
      sqlDate(0),
      over.relevance ?? 0.9,
      over.signal_type ?? null,
      over.signal_priority ?? null,
    );
  return result.lastInsertRowid as number;
}

function entry(over: Partial<VulnerabilityEntry> = {}): VulnerabilityEntry {
  return {
    package: "jsonwebtoken",
    currentVersion: "9.3.1",
    ecosystem: "crates.io",
    isDev: false,
    isDirect: true,
    devScopeKnown: true,
    vulnId: "GHSA-h395-gr6q-cpjc",
    aliases: ["RUSTSEC-2026-0088"],
    severity: "medium",
    cvssScore: 5.3,
    summary: "jsonwebtoken has Type Confusion that leads to potential authorization bypass",
    fixedVersion: "10.3.0",
    published: "2026-02-03T18:47:40Z",
    references: ["https://github.com/advisories/GHSA-h395-gr6q-cpjc"],
    target: null,
    platformActive: true,
    sourceDirs: ["d:/proj/relay"],
    ...over,
  };
}

function scanOf(entries: VulnerabilityEntry[], over: Partial<VulnerabilityScanResult> = {}): VulnerabilityScanResult {
  const bySeverity = { critical: 0, high: 0, medium: 0, low: 0, unknown: 0 };
  for (const e of entries) bySeverity[e.severity]++;
  return {
    scannedAt: new Date().toISOString(),
    projectPath: "d:/proj",
    ecosystemsScanned: ["crates.io"],
    totalScanned: 12,
    totalVulnerable: new Set(entries.map((e) => e.package)).size,
    platformInactiveVulnerable: new Set(entries.filter((e) => !e.platformActive).map((e) => e.package)).size,
    bySeverity,
    vulnerabilities: entries,
    cleanCount: 12 - entries.length,
    scanDurationMs: 5,
    cached: false,
    offline: false,
    ...over,
  };
}

/** A live layer whose scan state is under the test's control. */
function stubIntel(
  scan: VulnerabilityScanResult | null,
  over: Partial<BriefingLiveIntel> = {},
): BriefingLiveIntel {
  return {
    isEnabled: () => true,
    getProjectRoot: () => "d:/proj",
    getHeadlines: () => [],
    getVulnerabilities: () => scan,
    ensureVulnerabilities: async () => scan,
    ...over,
  };
}

const TASK = "Upgrade jsonwebtoken in the relay service and review auth token verification";

// ---------------------------------------------------------------------------
// what_should_i_know
// ---------------------------------------------------------------------------

describe("what_should_i_know — the briefing never answers safe without a scan", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  it("cold start: an unavailable scan yields delegation unknown, never safe", async () => {
    // The live defect: the scan had not run, the cache read nothing, and the
    // verdict was safe_to_delegate.
    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(null));

    expect(result.scan_status).toBe("unavailable");
    expect(result.delegation_assessment.level).toBe("unknown");
    expect(result.delegation_assessment.reason).toContain("Vulnerability scan unavailable");
    expect(result.delegation_assessment.reason).toContain("not as safe");
    expect(result.summary).toContain("unavailable");
  });

  it("a ready scan holding a medium jsonwebtoken advisory surfaces it", async () => {
    const scan = scanOf([entry()]);
    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(scan));

    expect(result.scan_status).toBe("ready");
    const named = result.advisories.filter((a) => /jsonwebtoken/i.test(`${a.title} ${a.action}`));
    expect(named.length).toBeGreaterThan(0);
    // The per-vulnerability live signal carries the upgrade target.
    expect(result.advisories.some((a) => a.action.includes("Upgrade jsonwebtoken to 10.3.0"))).toBe(true);
    // A medium advisory in the very package being upgraded is not "safe".
    expect(result.delegation_assessment.level).toBe("review_needed");
  });

  it("safe_to_delegate is only emitted over a ready, clean scan", async () => {
    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(scanOf([])));

    expect(result.scan_status).toBe("ready");
    expect(result.delegation_assessment.level).toBe("safe_to_delegate");
  });

  it("disabled live intelligence reports scan_status disabled and delegation unknown", async () => {
    const offline = await executeWhatShouldIKnow(
      db,
      { task: TASK },
      stubIntel(null, { isEnabled: () => false }),
    );
    expect(offline.scan_status).toBe("disabled");
    expect(offline.delegation_assessment.level).toBe("unknown");

    const absent = await executeWhatShouldIKnow(db, { task: TASK }, null);
    expect(absent.scan_status).toBe("disabled");
    expect(absent.delegation_assessment.level).toBe("unknown");
  });

  it("a scan that resolved no dependency versions is unavailable, not ready", async () => {
    const empty = scanOf([], { totalScanned: 0, cleanCount: 0 });
    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(empty));

    expect(result.scan_status).toBe("unavailable");
    expect(result.delegation_assessment.level).toBe("unknown");
  });

  it("security evidence already in hand still yields human_only when the scan is unavailable", async () => {
    insertItem(db, {
      title: "CRITICAL: jsonwebtoken authorization bypass in relay auth",
      hoursAgo: 2,
      signal_type: "security_alert",
      signal_priority: "critical",
    });

    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(null));

    expect(result.scan_status).toBe("unavailable");
    expect(result.delegation_assessment.level).toBe("human_only");
  });

  it("platform-inactive and maintenance rows never drive the verdict", async () => {
    const scan = scanOf([
      entry({ package: "nix", vulnId: "GHSA-inactive", aliases: [], severity: "critical", platformActive: false, target: "cfg(unix)" }),
      entry({ package: "paste", vulnId: "RUSTSEC-2024-0436", aliases: [], severity: "unknown", summary: "paste - no longer maintained", fixedVersion: null }),
    ]);
    const result = await executeWhatShouldIKnow(db, { task: "Upgrade nix and paste" }, stubIntel(scan));

    expect(result.scan_status).toBe("ready");
    expect(result.advisories.some((a) => /\bnix\b|\bpaste\b/i.test(`${a.title} ${a.action}`))).toBe(false);
    expect(result.advisories.some((a) => a.title.includes("known vulnerabilities"))).toBe(false);
    expect(result.delegation_assessment.level).toBe("safe_to_delegate");
  });

  it("a security alert older than 72 hours but inside 30 days reaches the briefing", async () => {
    // A recent unrelated item keeps the 72-hour pass from falling back to the
    // deep window on its own; the 30-day security pass is what must find it.
    insertItem(db, { title: "Rust 1.99 released", hoursAgo: 3, source_type: "hackernews", relevance: 0.5 });
    insertItem(db, {
      title: "hono 4.12.34 patches middleware bypass",
      hoursAgo: 10 * 24,
      signal_type: "security_alert",
      signal_priority: "medium",
    });

    const result = await executeWhatShouldIKnow(
      db,
      { task: "Upgrade hono in the API gateway" },
      stubIntel(scanOf([])),
    );

    expect(result.advisories.some((a) => a.title.includes("hono 4.12.34"))).toBe(true);
  });

  it("advisories that appear in both passes are reported once", async () => {
    insertItem(db, {
      title: "CRITICAL: jsonwebtoken authorization bypass in relay auth",
      hoursAgo: 2,
      signal_type: "security_alert",
      signal_priority: "critical",
    });
    const result = await executeWhatShouldIKnow(db, { task: TASK }, stubIntel(scanOf([entry()])));

    const dbRow = result.advisories.filter((a) => a.title.includes("CRITICAL: jsonwebtoken authorization bypass"));
    expect(dbRow).toHaveLength(1);
    const liveRow = result.advisories.filter((a) => a.action.includes("Upgrade jsonwebtoken to 10.3.0"));
    expect(liveRow).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// LiveIntelligence warmup / ensure
// ---------------------------------------------------------------------------

describe("LiveIntelligence.ensureVulnerabilities", () => {
  const dep: ResolvedDependency = {
    name: "jsonwebtoken", version: "9.3.1", ecosystem: "crates.io", isDev: false, isDirect: true,
    devScopeKnown: true, target: null, platformActive: true, sourceDirs: ["d:/proj/relay"],
  };
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  function enabledIntel(scanImpl: () => Promise<VulnerabilityScanResult>): LiveIntelligence {
    const prior = process.env.FOURDA_OFFLINE;
    delete process.env.FOURDA_OFFLINE;
    const li = new LiveIntelligence(new Database(":memory:"));
    if (prior !== undefined) process.env.FOURDA_OFFLINE = prior;
    const priv = li as unknown as { auditDeps: ResolvedDependency[]; osvScanner: { scan: () => Promise<VulnerabilityScanResult> } };
    priv.auditDeps = [dep];
    priv.osvScanner = { scan: scanImpl };
    return li;
  }

  it("returns null immediately when live intelligence is disabled", async () => {
    process.env.FOURDA_OFFLINE = "true";
    try {
      const li = new LiveIntelligence(new Database(":memory:"));
      expect(await li.ensureVulnerabilities("d:/proj", 1000)).toBeNull();
    } finally {
      delete process.env.FOURDA_OFFLINE;
    }
  });

  it("waits for the warmup and returns the stored scan; a timed-out wait keeps the warmup alive", async () => {
    const scan = scanOf([entry()]);
    const li = enabledIntel(async () => {
      await sleep(60);
      return scan;
    });
    li.startVulnerabilityWarmup("d:/proj");

    // Before the fix there was no way to wait: the cache read null and that
    // was the answer.
    expect(li.getVulnerabilities()).toBeNull();
    expect(await li.ensureVulnerabilities("d:/proj", 5)).toBeNull();

    const ready = await li.ensureVulnerabilities("d:/proj", 2000);
    expect(ready).not.toBeNull();
    expect(ready!.vulnerabilities[0].package).toBe("jsonwebtoken");
    expect(li.getVulnerabilities()).toBe(ready);
  });

  it("starts a warmup itself when none is running", async () => {
    const li = enabledIntel(async () => scanOf([entry()]));
    const result = await li.ensureVulnerabilities("d:/proj", 2000);
    expect(result?.totalVulnerable).toBe(1);
  });

  it("a scan that throws resolves to null and never throws itself", async () => {
    const li = enabledIntel(async () => {
      throw new Error("OSV API error: 503");
    });
    li.startVulnerabilityWarmup("d:/proj");
    await expect(li.ensureVulnerabilities("d:/proj", 2000)).resolves.toBeNull();
    expect(li.getVulnerabilities()).toBeNull();
  });

  it("an offline scan result is not a usable scan", async () => {
    const li = enabledIntel(async () => scanOf([], { offline: true, cleanCount: 0 }));
    expect(await li.ensureVulnerabilities("d:/proj", 2000)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Server wiring — index.ts cannot be imported (it starts the server), so the
// contract is pinned on its source: both database modes start the warmup.
// ---------------------------------------------------------------------------

describe("server init wires the vulnerability warmup in both modes", () => {
  const indexSource = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "..", "index.ts"),
    "utf8",
  );

  it("starts the warmup in the standalone AND the full-database branch", () => {
    const starts = indexSource.match(/startVulnerabilityWarmup\(/g) ?? [];
    expect(starts.length).toBeGreaterThanOrEqual(2);
  });

  it("no branch calls scanVulnerabilities directly (the warmup owns the scan)", () => {
    expect(indexSource).not.toMatch(/\.scanVulnerabilities\(/);
  });
});

// ---------------------------------------------------------------------------
// get_actionable_signals — live injection
// ---------------------------------------------------------------------------

describe("get_actionable_signals — one signal per vulnerability", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  const ghsa = entry({ vulnId: "GHSA-h395-gr6q-cpjc", aliases: ["CVE-2026-25800", "RUSTSEC-2026-0088"] });
  const rustsec = entry({ vulnId: "RUSTSEC-2026-0088", aliases: ["GHSA-h395-gr6q-cpjc"] });

  it("clusters alias-connected records, preferring the GHSA id", () => {
    const clusters = clusterVulnerabilities([rustsec, ghsa]);
    expect(clusters).toHaveLength(1);
    expect(clusters[0].representative.vulnId).toBe("GHSA-h395-gr6q-cpjc");
    expect(clusters[0].ids).toEqual(
      expect.arrayContaining(["GHSA-h395-gr6q-cpjc", "CVE-2026-25800", "RUSTSEC-2026-0088"]),
    );
  });

  it("connects transitively (A knows B, B knows C) and keeps distinct scopes apart", () => {
    const a = entry({ vulnId: "A-1", aliases: ["B-1"] });
    const b = entry({ vulnId: "B-1", aliases: ["C-1"] });
    const c = entry({ vulnId: "C-1", aliases: [] });
    expect(clusterVulnerabilities([a, b, c])).toHaveLength(1);

    const otherPackage = entry({ vulnId: "A-1", aliases: ["B-1"], package: "other" });
    const otherVersion = entry({ vulnId: "A-1", aliases: ["B-1"], currentVersion: "8.0.0" });
    expect(clusterVulnerabilities([a, otherPackage, otherVersion])).toHaveLength(3);

    const unrelated = [entry({ vulnId: "X-1", aliases: [] }), entry({ vulnId: "Y-1", aliases: [] })];
    expect(clusterVulnerabilities(unrelated)).toHaveLength(2);
  });

  it("emits ONE security signal for a GHSA/RUSTSEC pair, listing every id in triggers", () => {
    const live = { getVulnerabilities: () => scanOf([ghsa, rustsec]) };
    const { signals } = executeGetActionableSignals(db, { limit: 50 }, live);

    const liveRows = signals.filter((s) => s.source_type === "osv_live");
    expect(liveRows).toHaveLength(1);
    expect(liveRows[0].triggers).toEqual(
      expect.arrayContaining(["GHSA-h395-gr6q-cpjc", "CVE-2026-25800", "RUSTSEC-2026-0088"]),
    );
    expect(liveRows[0].signal_priority).toBe("medium");
    expect(liveRows[0].action).toBe("Upgrade jsonwebtoken to 10.3.0");
  });

  it("platform-inactive advisories drop to low with a host note", () => {
    const live = {
      getVulnerabilities: () => scanOf([
        entry({ package: "nix", vulnId: "GHSA-inactive", aliases: [], severity: "critical", platformActive: false, target: "cfg(unix)", fixedVersion: "0.30.0" }),
      ]),
    };
    const { signals } = executeGetActionableSignals(db, { limit: 50 }, live);
    const row = signals.find((s) => s.source_type === "osv_live");
    expect(row).toBeDefined();
    expect(row!.signal_priority).toBe("low");
    expect(row!.action).toBe("Not built on this host — Upgrade nix to 0.30.0");
  });

  it("maintenance notices drop to low with no fix to apply", () => {
    const live = {
      getVulnerabilities: () => scanOf([
        entry({ package: "paste", vulnId: "RUSTSEC-2024-0436", aliases: [], severity: "unknown", summary: "paste - no longer maintained", fixedVersion: null }),
      ]),
    };
    const { signals } = executeGetActionableSignals(db, { limit: 50 }, live);
    const row = signals.find((s) => s.source_type === "osv_live");
    expect(row).toBeDefined();
    expect(row!.signal_priority).toBe("low");
    expect(row!.action).toBe("Maintenance notice — no fix to apply");
    expect(row!.title).toContain("MAINTENANCE:");
  });

  it("maps the pipeline's stored priority vocabulary onto the tool's tiers", () => {
    // The desktop pipeline writes critical / alert / advisory / watch; the
    // reader cast the string through unchanged, so 92 of 93 stamped rows in
    // the live corpus carried a priority no filter or sort recognised.
    expect(normalizeStoredPriority("critical")).toBe("critical");
    expect(normalizeStoredPriority("alert")).toBe("high");
    expect(normalizeStoredPriority("advisory")).toBe("medium");
    expect(normalizeStoredPriority("watch")).toBe("low");
    expect(normalizeStoredPriority("high")).toBe("high");
    expect(normalizeStoredPriority("something-new")).toBe("medium");

    const alert = insertItem(db, { title: "hono middleware bypass fixed in 4.12.34", hoursAgo: 2, signal_type: "security_alert", signal_priority: "alert" });
    const advisory = insertItem(db, { title: "tokio 1.51 scheduler regression notes", hoursAgo: 2, signal_type: "breaking_change", signal_priority: "advisory" });
    const watch = insertItem(db, { title: "serde 1.0.220 minor release", hoursAgo: 2, signal_type: "tech_trend", signal_priority: "watch" });

    const { signals } = executeGetActionableSignals(db, { limit: 50 }, null);
    const byId = new Map(signals.map((s) => [s.id, s.signal_priority]));
    expect(byId.get(alert)).toBe("high");
    expect(byId.get(advisory)).toBe("medium");
    expect(byId.get(watch)).toBe("low");

    const highOnly = executeGetActionableSignals(db, { priority_filter: "high", limit: 50 }, null);
    expect(highOnly.signals.map((s) => s.id)).toEqual([alert]);
  });

  it("a signal_type filter reaches stored matches the top-200 general read would drop", () => {
    // Live 2026-09-07: the two in-window stored security alerts ranked 299th
    // and 582nd across all types, so a security-only pass returned nothing.
    const words = [
      ["amber", "basalt", "cobalt", "dune", "ember", "fjord", "garnet"],
      ["harbor", "isotope", "juniper", "kestrel", "lantern", "meadow", "nebula", "orchid", "pylon", "quartz", "raven"],
      ["saffron", "tundra", "umbra", "velvet", "willow", "xenon", "yarrow", "zephyr", "anvil", "bramble", "cinder", "drift", "estuary"],
    ];
    for (let i = 0; i < 250; i++) {
      insertItem(db, {
        title: `${words[0][i % 7]} ${words[1][i % 11]} ${words[2][i % 13]} story ${i}`,
        hoursAgo: 5,
        source_type: "hackernews",
        relevance: 0.9,
      });
    }
    const buried = insertItem(db, {
      title: "hono 4.12.34 patches middleware bypass",
      hoursAgo: 5,
      relevance: 0.5,
      signal_type: "security_alert",
      signal_priority: "medium",
    });

    const { signals } = executeGetActionableSignals(db, { signal_type: "security_alert", since_hours: 720, limit: 50 }, null);
    expect(signals.map((s) => s.id)).toContain(buried);
  });

  it("honours a 30-day since_hours window instead of clamping it to seven days", () => {
    insertItem(db, { title: "Rust 1.99 released", hoursAgo: 3, source_type: "hackernews", relevance: 0.5 });
    const old = insertItem(db, {
      title: "hono 4.12.34 patches middleware bypass",
      hoursAgo: 10 * 24,
      signal_type: "security_alert",
      signal_priority: "medium",
    });

    const { signals } = executeGetActionableSignals(db, { since_hours: 720, limit: 50 }, null);
    expect(signals.map((s) => s.id)).toContain(old);
  });
});

// ---------------------------------------------------------------------------
// 5.1.0 — one severity rule with the app, and install drift in the briefing
// ---------------------------------------------------------------------------

describe("the briefing and live signals grade by the shared rule and name install drift", () => {
  let db: FourDADatabase;

  beforeEach(() => {
    db = createTestDatabase();
  });

  afterEach(() => {
    db.close();
  });

  // Measured 2026-09-10: this read "CRITICAL: Sandbox Breakout" in the
  // briefing while the app graded the same advisory High.
  const sandbox = entry({
    package: "sandbox",
    currentVersion: "3.1.2",
    ecosystem: "npm",
    isDirect: false,
    devScopeKnown: false,
    vulnId: "GHSA-sbx-0001",
    aliases: [],
    severity: "critical",
    cvssScore: 9.8,
    summary: "Sandbox Breakout",
    fixedVersion: "3.1.3",
    sourceDirs: ["d:/proj/paddle-webhook"],
  });

  it("get_actionable_signals presents a transitive critical of unknown scope as HIGH, and says why", () => {
    const { signals } = executeGetActionableSignals(db, { limit: 50 }, { getVulnerabilities: () => scanOf([sandbox]) });
    const row = signals.find((s) => s.source_type === "osv_live");
    expect(row).toBeDefined();
    expect(row!.signal_priority).toBe("high");
    expect(row!.title.startsWith("HIGH: Sandbox Breakout")).toBe(true);
    expect(row!.action).toContain("the advisory rates it critical; graded high as a transitive-only dependency");
  });

  it("what_should_i_know no longer calls it critical", async () => {
    const result = await executeWhatShouldIKnow(db, { task: "Tidy the README" }, stubIntel(scanOf([sandbox])));
    const summaryRow = result.advisories.find((a) => a.title.includes("known vulnerabilities"));
    expect(summaryRow?.priority).toBe("high");
    expect(result.advisories.some((a) => /CRITICAL/.test(a.title))).toBe(false);
  });

  // The installed copy in node_modules, not what the lockfile pins (hono
  // 4.13.1 installed under a 4.13.5 lockfile for 25 days).
  const drifted = entry({
    package: "hono",
    currentVersion: "4.13.1",
    ecosystem: "npm",
    vulnId: "GHSA-hono-0003",
    aliases: ["CVE-2026-84365"],
    severity: "medium",
    cvssScore: 5.3,
    summary: "hono: cookie parsing",
    fixedVersion: "4.13.4",
    sourceDirs: ["d:/proj/mcp-4da-server"],
    installDriftOf: "4.13.5",
    installFix: "pnpm install",
  });

  it("a vulnerable installed copy yields at least review_needed and names the reinstall, whatever the task", async () => {
    const result = await executeWhatShouldIKnow(db, { task: "Tidy the README" }, stubIntel(scanOf([drifted])));

    expect(result.scan_status).toBe("ready");
    const drift = result.advisories.find((a) => a.title.startsWith("hono: the installed 4.13.1"));
    expect(drift).toBeDefined();
    expect(drift!.signal_type).toBe("security_alert");
    expect(drift!.action).toContain("Run `pnpm install` in d:/proj/mcp-4da-server");
    expect(drift!.action).toContain("the lockfile version is not affected");
    // Graded like the app's install-drift row: the reinstall clears an
    // advisory the running copy has, so high, though the advisory is medium.
    expect(drift!.priority).toBe("high");
    expect(["review_needed", "human_only"]).toContain(result.delegation_assessment.level);
    expect(result.summary).not.toContain("No active advisories");
  });

  it("drift whose lockfile version is exposed too is medium, and says to upgrade before reinstalling", async () => {
    const lockfileRow = entry({
      ...drifted,
      currentVersion: "4.13.5",
      installDriftOf: undefined,
      installFix: undefined,
    });
    const result = await executeWhatShouldIKnow(
      db,
      { task: "Tidy the README" },
      stubIntel(scanOf([drifted, lockfileRow])),
    );

    const drift = result.advisories.find((a) => a.title.startsWith("hono: the installed 4.13.1"));
    expect(drift).toBeDefined();
    expect(drift!.priority).toBe("medium");
    expect(drift!.action).toContain("the lockfile version is also affected; upgrade it, then reinstall");
    expect(result.delegation_assessment.level).toBe("review_needed");
  });

  it("get_actionable_signals advises the reinstall, not an upgrade, for the installed copy", () => {
    const { signals } = executeGetActionableSignals(db, { limit: 50 }, { getVulnerabilities: () => scanOf([drifted]) });
    const row = signals.find((s) => s.source_type === "osv_live");
    expect(row!.action).toBe(
      "Run `pnpm install` in d:/proj/mcp-4da-server — node_modules has hono@4.13.1; the lockfile pins 4.13.5",
    );
  });

  it("the briefing's scan block says when the versions it covers were resolved", async () => {
    const result = await executeWhatShouldIKnow(
      db,
      { task: TASK },
      stubIntel(scanOf([entry()]), {
        refreshIfLockfilesChanged: () => true,
        getResolutionProvenance: () => ({ resolvedAt: "2026-09-10T21:43:00.000Z", sources: [] }),
      }),
    );
    expect(result.scan).toEqual({
      status: "ready",
      scanned_at: expect.any(String),
      resolved_at: "2026-09-10T21:43:00.000Z",
      re_resolved_this_call: true,
    });
    expect(result.scan_status).toBe("ready");
  });
});
