// SPDX-License-Identifier: Apache-2.0
/**
 * knowledge_gaps says what the desktop app says.
 *
 * Measured 2026-09-11 on the founder machine: the live `knowledge_gaps` tool
 * returned five `critical` gaps (vitest, jsonwebtoken, rsa, axios, socket.io).
 * Four were false or inflated, and every one disagreed with
 * `knowledge_decay::detect_knowledge_gaps`, which computes the same concept in
 * Rust. Each block reproduces one measured case with rows copied from that
 * database: item ids, titles, affected ranges, CVSS scores and labels are
 * verbatim unless a comment says the row was constructed.
 */
import { describe, it, expect, afterEach } from "vitest";
import Database from "better-sqlite3";
import { FourDADatabase } from "../db.js";
import { executeKnowledgeGaps, gradeGap, versionInAnyRange } from "../tools/knowledge-gaps.js";
import { advisoryTier, parseAffectedRanges } from "../tools/knowledge-gap-ranges.js";
import { compareVersionPrecedence } from "../live/semver-precedence.js";
import { compareSemver } from "../live/semver-utils.js";
import type { ResolvedDependency } from "../live/types.js";

// The tables the tool reads, in the desktop app's shape.
const CORE_SCHEMA = `
  CREATE TABLE source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT NOT NULL, source_id TEXT, url TEXT,
    title TEXT, content TEXT, content_type TEXT, created_at TEXT, published_at TEXT, relevance_score REAL
  );
  CREATE TABLE project_dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT, project_path TEXT NOT NULL, package_name TEXT NOT NULL,
    version TEXT, language TEXT NOT NULL, is_direct INTEGER DEFAULT 1, is_dev INTEGER DEFAULT 0
  );
  CREATE TABLE user_dependencies (
    id INTEGER PRIMARY KEY, project_path TEXT NOT NULL, package_name TEXT NOT NULL, version TEXT,
    ecosystem TEXT NOT NULL, is_dev INTEGER DEFAULT 0, is_direct INTEGER DEFAULT 1,
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now')), UNIQUE(project_path, package_name, ecosystem)
  );
  CREATE TABLE interactions (item_id INTEGER, action_type TEXT);
  CREATE TABLE feedback (id INTEGER PRIMARY KEY, source_item_id INTEGER, relevant INTEGER, created_at TEXT);
  CREATE TABLE osv_advisories (
    id INTEGER PRIMARY KEY AUTOINCREMENT, advisory_id TEXT NOT NULL, package_name TEXT NOT NULL,
    ecosystem TEXT NOT NULL, affected_ranges TEXT, cvss_score REAL, severity_label TEXT,
    aliases TEXT, withdrawn_at TEXT, UNIQUE(advisory_id, package_name, ecosystem)
  );
  CREATE TABLE git_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT, repo_path TEXT NOT NULL, commit_hash TEXT,
    timestamp TEXT DEFAULT (datetime('now'))
  );
`;

// The dependency linker's table. A standalone database has none and falls
// back to the advisory's subject package.
const LINKER_SCHEMA = `
  CREATE TABLE source_item_dependencies (
    id INTEGER PRIMARY KEY, source_item_id INTEGER NOT NULL, package_name TEXT NOT NULL,
    ecosystem TEXT, match_type TEXT NOT NULL DEFAULT 'title_heuristic', confidence REAL NOT NULL DEFAULT 0.5
  );
`;

const opened: FourDADatabase[] = [];
afterEach(() => {
  for (const db of opened.splice(0)) db.close();
});

function createDb({ linker = true }: { linker?: boolean } = {}): FourDADatabase {
  const raw = new Database(":memory:");
  raw.exec(CORE_SCHEMA);
  if (linker) raw.exec(LINKER_SCHEMA);
  const db = Object.create(FourDADatabase.prototype) as FourDADatabase;
  (db as unknown as { db: Database.Database }).db = raw;
  opened.push(db);
  return db;
}

/** A declaring project; `version` also records its lockfile-resolved install. */
function declare(db: FourDADatabase, project: string, pkg: string, language: string, version?: string): void {
  const raw = db.getRawDb();
  raw
    .prepare("INSERT INTO project_dependencies (project_path, package_name, version, language) VALUES (?, ?, NULL, ?)")
    .run(project, pkg, language);
  if (version) {
    raw
      .prepare("INSERT INTO user_dependencies (project_path, package_name, version, ecosystem) VALUES (?, ?, ?, ?)")
      .run(project, pkg, version, language);
  }
}

interface AdvisoryFixture {
  id: string;
  pkg: string;
  ecosystem: string;
  ranges: string | null;
  cvss?: number | null;
  label?: string | null;
  aliases?: string[];
}

function storeAdvisory(db: FourDADatabase, a: AdvisoryFixture): void {
  db.getRawDb()
    .prepare(
      `INSERT INTO osv_advisories (advisory_id, package_name, ecosystem, affected_ranges, cvss_score, severity_label, aliases)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(a.id, a.pkg, a.ecosystem, a.ranges, a.cvss ?? null, a.label ?? null, JSON.stringify(a.aliases ?? []));
}

interface ItemFixture {
  id?: number;
  title: string;
  source_type: string;
  source_id?: string | null;
  content?: string;
  content_type?: string | null;
  daysAgo?: number;
  published?: string | null;
  relevance?: number;
}

function addItem(db: FourDADatabase, i: ItemFixture): number {
  const result = db
    .getRawDb()
    .prepare(
      `INSERT INTO source_items (id, source_type, source_id, url, title, content, content_type, created_at, published_at, relevance_score)
       VALUES (?, ?, ?, 'https://example.com', ?, ?, ?, datetime('now', ?), ?, ?)`,
    )
    .run(
      i.id ?? null,
      i.source_type,
      i.source_id ?? null,
      i.title,
      i.content ?? i.title,
      i.content_type ?? null,
      `-${i.daysAgo ?? 2} days`,
      i.published ?? null,
      i.relevance ?? 0.5,
    );
  return Number(result.lastInsertRowid);
}

/** A structured link from the dependency linker (`match_type = 'advisory'` unless given). */
function link(db: FourDADatabase, itemId: number, pkg: string, matchType = "advisory"): void {
  db.getRawDb()
    .prepare(
      "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence) VALUES (?, ?, 'advisory', ?, 0.9)",
    )
    .run(itemId, pkg, matchType);
}

/** A commit recorded by the git scanner; `repo_path` is stored raw, as it is live ("D:\4DA"). */
function commit(db: FourDADatabase, repoPath: string, daysAgo: number): void {
  db.getRawDb()
    .prepare("INSERT INTO git_signals (repo_path, commit_hash, timestamp) VALUES (?, 'c0ffee', datetime('now', ?))")
    .run(repoPath, `-${daysAgo} days`);
}

function gapFor(db: FourDADatabase, dependency: string, minSeverity = "low") {
  return executeKnowledgeGaps(db, { min_severity: minSeverity, limit: 50 }).gaps?.find((g) => g.dependency === dependency);
}

const FIXED_AT = (version: string) => `[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"${version}"}]}]`;

/** vitest's three stored advisories, verbatim from `osv_advisories`. */
const VITEST_ADVISORIES: AdvisoryFixture[] = [
  {
    id: "GHSA-9crc-q9x8-hgqq",
    pkg: "vitest",
    ecosystem: "npm",
    cvss: 9.6,
    label: "critical",
    aliases: ["CVE-2025-24964"],
    ranges:
      '[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"fixed":"1.6.1"}]},{"type":"SEMVER","events":[{"introduced":"2.0.0"},{"fixed":"2.1.9"}]},{"type":"SEMVER","events":[{"introduced":"3.0.0"},{"fixed":"3.0.5"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"last_affected":"0.0.125"}]}]',
  },
  {
    id: "GHSA-82fw-gwwq-j7x9",
    pkg: "vitest",
    ecosystem: "npm",
    cvss: 5.9,
    label: "medium",
    aliases: ["CVE-2026-84373"],
    ranges:
      '[{"type":"SEMVER","events":[{"introduced":"2.1.0"},{"fixed":"4.1.11"}]},{"type":"SEMVER","events":[{"introduced":"5.0.0-beta.1"},{"fixed":"5.0.0-rc.2"}]}]',
  },
  {
    id: "GHSA-5xrq-8626-4rwp",
    pkg: "vitest",
    ecosystem: "npm",
    cvss: 9.8,
    label: "critical",
    aliases: ["CVE-2026-47429"],
    ranges:
      '[{"type":"SEMVER","events":[{"introduced":"4.0.0"},{"fixed":"4.1.0"}]},{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"3.2.6"}]}]',
  },
];

/** id 84937, verbatim: the row the live tool graded critical on vitest 4.1.11. */
const VITEST_CVE_ROW: ItemFixture = {
  id: 84937,
  title: "[CVE-2026-84373] Vitest: Path Traversal / Arbitrary File Read via @vitest/mocker Redirect Mock",
  source_type: "cve",
  source_id: "CVE-2026-84373",
  content_type: "security_advisory",
  relevance: 0.3700000047683716,
};

/** An osv row for GHSA-9crc itself, constructed in the ingested "[ID] package: summary" shape. */
const VITEST_9CRC_ROW: ItemFixture = {
  title:
    "[GHSA-9crc-q9x8-hgqq] vitest: Vitest allows Remote Code Execution when accessing a malicious website while Vitest API server is listening",
  source_type: "osv",
  source_id: "GHSA-9crc-q9x8-hgqq",
  content_type: "security_advisory",
  relevance: 0.37,
};

describe("(a) vitest 4.1.11: last_affected is an inclusive upper bound, not an open range", () => {
  const rangesOf = (id: string) => parseAffectedRanges(VITEST_ADVISORIES.find((a) => a.id === id)?.ranges) ?? [];

  it("no stored vitest range contains 4.1.11, and GHSA-9crc still holds its pre-1.0 window", () => {
    for (const a of VITEST_ADVISORIES) expect(versionInAnyRange(rangesOf(a.id), "4.1.11")).toBe(false);
    expect(versionInAnyRange(rangesOf("GHSA-9crc-q9x8-hgqq"), "0.0.100")).toBe(true);
    expect(versionInAnyRange(rangesOf("GHSA-9crc-q9x8-hgqq"), "0.0.125")).toBe(true);
    expect(versionInAnyRange(rangesOf("GHSA-9crc-q9x8-hgqq"), "0.0.126")).toBe(false);
    // The one real exposure, fixed at exactly 4.1.11.
    expect(versionInAnyRange(rangesOf("GHSA-82fw-gwwq-j7x9"), "4.1.10")).toBe(true);
  });

  for (const linker of [true, false]) {
    it(`drops both advisory rows at 4.1.11 (${linker ? "linker-proven" : "standalone subject"} citations)`, () => {
      const db = createDb({ linker });
      declare(db, "d:/4da", "vitest", "javascript", "4.1.11");
      for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
      const cve = addItem(db, VITEST_CVE_ROW);
      const osv = addItem(db, VITEST_9CRC_ROW);
      if (linker) {
        link(db, cve, "vitest");
        link(db, osv, "vitest");
      }
      expect(gapFor(db, "vitest")).toBeUndefined();
    });
  }

  it("judges a row the mirror cannot resolve on the package, which is now safe", () => {
    // Before the fix the trailing `introduced: "0"` put every vitest version
    // inside GHSA-9crc, and this row graded critical.
    const db = createDb();
    declare(db, "d:/4da", "vitest", "javascript", "4.1.11");
    for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
    link(db, addItem(db, { ...VITEST_CVE_ROW, id: undefined, source_id: "CVE-2026-99999" }), "vitest");
    expect(gapFor(db, "vitest")?.gap_severity).toBe("low");
  });

  it("still reaches an install at 0.0.100, at the advisory's own critical tier", () => {
    const db = createDb();
    declare(db, "d:/legacy", "vitest", "javascript", "0.0.100");
    for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
    link(db, addItem(db, VITEST_9CRC_ROW), "vitest");
    const gap = gapFor(db, "vitest");
    expect(gap?.gap_severity).toBe("critical");
    expect(gap?.version).toBe("0.0.100");
  });
});

/** GHSA-h395-gr6q-cpjc, verbatim: GitHub MODERATE, CVSS v4 (score NULL in the mirror). */
const H395: AdvisoryFixture = {
  id: "GHSA-h395-gr6q-cpjc",
  pkg: "jsonwebtoken",
  ecosystem: "crates.io",
  ranges: FIXED_AT("10.3.0"),
  cvss: null,
  label: "medium",
  aliases: ["CVE-2026-25537"],
};

/** id 71038, verbatim. */
const H395_ROW: ItemFixture = {
  id: 71038,
  title: "[GHSA-h395-gr6q-cpjc] jsonwebtoken: jsonwebtoken has Type Confusion that leads to potential authorization bypass",
  source_type: "osv",
  source_id: "GHSA-h395-gr6q-cpjc",
  content_type: "security_advisory",
  published: "2026-02-03 18:47:40",
  relevance: 0.9001746773719788,
};

describe("(b) jsonwebtoken 9.3.1 (relay): the advisory's own tier, the exposed project only", () => {
  it("grades the medium GHSA-h395 high, never critical, and names relay alone", () => {
    const db = createDb();
    declare(db, "d:/4da/relay", "jsonwebtoken", "rust", "9.3.1");
    declare(db, "d:/4da/src-tauri", "jsonwebtoken", "rust", "10.4.0");
    declare(db, "d:/4da/editors/vscode/4da", "jsonwebtoken", "javascript", "9.0.3");
    storeAdvisory(db, H395);
    // The npm package's advisories (high, critical) must not grade the crate.
    storeAdvisory(db, { id: "GHSA-8cf7-32gw-wr33", pkg: "jsonwebtoken", ecosystem: "npm", ranges: FIXED_AT("9.0.0"), cvss: 8.1, label: "high", aliases: ["CVE-2022-23539"] });
    storeAdvisory(db, { id: "GHSA-c7hr-j4mj-j2w6", pkg: "jsonwebtoken", ecosystem: "npm", ranges: FIXED_AT("4.2.2"), label: "critical", aliases: ["CVE-2015-9235"] });
    link(db, addItem(db, H395_ROW), "jsonwebtoken");

    const gap = gapFor(db, "jsonwebtoken");
    expect(gap?.gap_severity).toBe("high");
    expect(gap?.project_path).toBe("d:/4da/relay");
    expect(gap?.version).toBe("9.3.1");
    expect(gap?.language).toBe("rust");
    expect(gapFor(db, "jsonwebtoken", "critical")).toBeUndefined();
  });
});

/** id 23957, verbatim: no source_item_dependencies row binds it to any package. */
const MAGICMIRROR_ROW: ItemFixture = {
  id: 23957,
  title: "[CVE-2026-63642] MagicMirror newsfeed Socket.IO notification allows blind server-side request forgery",
  source_type: "cve",
  source_id: "CVE-2026-63642",
  content_type: "security_advisory",
  relevance: 0.2986905872821808,
};

describe("(c) the MagicMirror CVE is not a socket.io citation", () => {
  for (const linker of [true, false]) {
    it(`cites nothing without a linker row (${linker ? "linker table present" : "standalone database"})`, () => {
      const db = createDb({ linker });
      declare(db, "d:/app", "socket.io", "javascript", "4.8.1");
      addItem(db, MAGICMIRROR_ROW);
      expect(gapFor(db, "socket.io")).toBeUndefined();
    });
  }

  it("a linker row is the proof that makes an advisory row a citation", () => {
    const db = createDb();
    declare(db, "d:/app", "socket.io", "javascript", "4.8.1");
    link(db, addItem(db, MAGICMIRROR_ROW), "socket.io");
    // Nothing stored about socket.io: exposure is unknown and stays conservative, ungraded.
    expect(gapFor(db, "socket.io")?.gap_severity).toBe("high");
  });
});

/** ids 77961 / 81015 / 83429 / 86646: one dev.to post, reposted daily. */
const REPOST_TITLE = "I built 59 free browser-based dev tools in vanilla JS — here's what I learned";
/** The line that matched `rsa`, 1,330 characters into every repost. */
const REPOST_BODY =
  "JWT Decoder — decode JWT header, payload, and check expiry locally RSA & ECC Key Generator — generate 2048-bit key pairs via SubtleCrypto Hash & Password tools";
const REPOSTS: Array<[number, number]> = [
  [77961, 5],
  [81015, 4],
  [83429, 3],
  [86646, 2],
];

describe("(d) editorial evidence is a title, and a repost is one story", () => {
  it("a mention in the body, not the title, cites nothing", () => {
    // The live reposts are typed show_and_tell, which the app also excludes;
    // typed as discussion here so the title rule alone is under test.
    const db = createDb();
    declare(db, "d:/4da/src-tauri", "rsa", "rust", "0.10.0-rc.18");
    for (const [id, daysAgo] of REPOSTS) {
      addItem(db, { id, title: REPOST_TITLE, source_type: "devto", content: REPOST_BODY, content_type: "discussion", daysAgo, relevance: 0.34 });
    }
    expect(gapFor(db, "rsa")).toBeUndefined();
  });

  it("reposts of one story collapse to the newest", () => {
    const db = createDb();
    declare(db, "d:/4da/src-tauri", "rsa", "rust", "0.10.0-rc.18");
    for (const [id, daysAgo] of REPOSTS) {
      // Case and punctuation differ between reposts; the normalized title does not.
      const title =
        daysAgo % 2 === 0
          ? "RSA 0.10.0-rc.19 Released — constant-time decryption lands"
          : "rsa 0.10.0-rc.19 released: constant-time decryption lands!";
      addItem(db, { id, title, source_type: "devto", content_type: "discussion", daysAgo });
    }
    const gap = gapFor(db, "rsa");
    expect(gap?.missed_items.map((m) => m.id)).toEqual([86646]);
    expect(gap?.missed_count).toBe(1);
    expect(gap?.gap_severity).toBe("medium");
  });

  it("drops the content types the app never counts", () => {
    const db = createDb();
    declare(db, "d:/4da", "vite", "javascript", "8.1.3");
    ["show_and_tell", "tutorial", "question", "help_request", "hiring", "clickbait"].forEach((contentType, i) => {
      addItem(db, { title: `vite 9.0.${i} breaking changes`, source_type: "devto", content_type: contentType });
    });
    expect(gapFor(db, "vite")).toBeUndefined();
  });

  it("drops an item the user already judged in the app", () => {
    const db = createDb();
    declare(db, "d:/4da", "vite", "javascript", "8.1.3");
    const id = addItem(db, { title: "vite 9.0.0 breaking changes", source_type: "rss" });
    expect(gapFor(db, "vite")?.gap_severity).toBe("high");
    db.getRawDb().prepare("INSERT INTO feedback (source_item_id, relevant) VALUES (?, 0)").run(id);
    expect(gapFor(db, "vite")).toBeUndefined();
  });
});

describe("(e) dormant projects: the app's active scope", () => {
  /** A real axios advisory title (id 38864); the mirror row is not needed here. */
  const AXIOS_ROW: ItemFixture = {
    title: "[GHSA-mwf2-3pr3-8698] axios: Axios: HTTP/2 streamed uploads bypass `maxBodyLength`",
    source_type: "osv",
    source_id: "GHSA-mwf2-3pr3-8698",
    content_type: "security_advisory",
  };

  it("skips a dependency whose projects are all dormant while another project is active", () => {
    const db = createDb();
    commit(db, "D:\\4DA", 1);
    declare(db, "c:/users/administrator/documents/kairos-mvp/backend", "axios", "javascript");
    link(db, addItem(db, AXIOS_ROW), "axios");
    declare(db, "d:/4da/src-tauri", "serde", "rust", "1.0.210");
    addItem(db, { title: "crates.io: serde v1.0.220", source_type: "crates_io" });

    expect(gapFor(db, "axios")).toBeUndefined();
    expect(gapFor(db, "serde")?.gap_severity).toBe("medium");
  });

  it("scopes nothing when git has recorded no activity", () => {
    const db = createDb();
    declare(db, "c:/users/administrator/documents/kairos-mvp/backend", "axios", "javascript");
    link(db, addItem(db, AXIOS_ROW), "axios");
    const gap = gapFor(db, "axios");
    // Unknown install, unresolvable row: conservative, and ungraded.
    expect(gap?.gap_severity).toBe("high");
    expect(gap?.version).toBeNull();
  });

  it("matches project paths on a path boundary, never a raw prefix", () => {
    const db = createDb();
    commit(db, "D:\\4DA", 1);
    declare(db, "d:/4da-experiments", "axios", "javascript");
    link(db, addItem(db, AXIOS_ROW), "axios");
    declare(db, "d:/4da", "serde", "rust", "1.0.210");
    expect(gapFor(db, "axios")).toBeUndefined();
  });

  it("judges a dependency on its active projects' installs only", () => {
    // navcal's untouched vitest 3.2.4 sits inside GHSA-82fw and the CVSS 9.8
    // GHSA-5xrq; the app's dependency funnel never lets that copy into the gap.
    const db = createDb();
    commit(db, "D:\\4DA", 1);
    declare(db, "d:/4da", "vitest", "javascript", "4.1.11");
    declare(db, "c:/users/administrator/documents/navcal", "vitest", "javascript", "3.2.4");
    for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
    link(db, addItem(db, VITEST_CVE_ROW), "vitest");
    expect(gapFor(db, "vitest")).toBeUndefined();
  });
});

/** ids 84993-84995 and the advisories their CVE ids alias, verbatim: all fixed at 4.13.5. */
const HONO_CVES = [
  { ghsa: "GHSA-crvj-82cr-hjcx", cve: "CVE-2026-84363", cvss: 5.9, row: 84995, relevance: 0.7819721102714539, title: "[CVE-2026-84363] Hono: Query parser reads parameters after the URL fragment, causing cache-key and proxy interpretation differentials" },
  { ghsa: "GHSA-g6gw-c38x-mqfc", cve: "CVE-2026-84364", cvss: 5.3, row: 84994, relevance: 0.89299476146698, title: "[CVE-2026-84364] Hono: Unbounded dot-notation nesting in `parseBody()` can cause memory exhaustion" },
  { ghsa: "GHSA-gqvv-2mrq-wpjv", cve: "CVE-2026-84365", cvss: 6.5, row: 84993, relevance: 0.7400000095367432, title: "[CVE-2026-84365] Hono: Incomplete fix for CVE-2026-39408: `toSSG()` still writes files outside the output directory" },
];

function seedHono(db: FourDADatabase, installed: string): void {
  declare(db, "d:/4da/mcp-4da-server", "hono", "javascript", installed);
  for (const h of HONO_CVES) {
    storeAdvisory(db, { id: h.ghsa, pkg: "hono", ecosystem: "npm", ranges: FIXED_AT("4.13.5"), cvss: h.cvss, label: "medium", aliases: [h.cve] });
    const id = addItem(db, { id: h.row, title: h.title, source_type: "cve", source_id: h.cve, content_type: "security_advisory", relevance: h.relevance });
    link(db, id, "hono");
  }
}

describe("(f) a cve row is judged on the advisory its CVE id aliases", () => {
  it("drops all three hono rows at 4.13.5, where every one is fixed", () => {
    const db = createDb();
    seedHono(db, "4.13.5");
    expect(gapFor(db, "hono")).toBeUndefined();
  });

  it("keeps all three at 4.13.3, at the advisories' own medium tier", () => {
    const db = createDb();
    seedHono(db, "4.13.3");
    const gap = gapFor(db, "hono");
    expect(gap?.gap_severity).toBe("high");
    expect(gap?.missed_items.map((m) => m.id).sort()).toEqual([84993, 84994, 84995]);
    expect(gap?.version).toBe("4.13.3");
  });
});

describe("(g) prereleases have precedence", () => {
  const range = parseAffectedRanges('[{"type":"SEMVER","events":[{"introduced":"5.0.0-beta.1"},{"fixed":"5.0.0-rc.2"}]}]') ?? [];

  it("puts 5.0.0-rc.1 inside [5.0.0-beta.1, 5.0.0-rc.2) and 5.0.0-rc.2 outside", () => {
    expect(versionInAnyRange(range, "5.0.0-rc.1")).toBe(true);
    expect(versionInAnyRange(range, "5.0.0-beta.1")).toBe(true);
    expect(versionInAnyRange(range, "5.0.0-beta.11")).toBe(true);
    expect(versionInAnyRange(range, "5.0.0-rc.2")).toBe(false);
    expect(versionInAnyRange(range, "5.0.0-alpha.9")).toBe(false);
    expect(versionInAnyRange(range, "5.0.0")).toBe(false);
  });

  it("orders versions the way semver.org section 11 does", () => {
    const ordered = [
      "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta",
      "1.0.0-beta.2", "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0",
    ];
    for (let i = 0; i + 1 < ordered.length; i++) {
      expect(compareVersionPrecedence(ordered[i], ordered[i + 1])).toBe(-1);
      expect(compareVersionPrecedence(ordered[i + 1], ordered[i])).toBe(1);
    }
    expect(compareVersionPrecedence("1.0.0+build.5", "1.0.0")).toBe(0);
    expect(compareVersionPrecedence("1.2", "1.2.0")).toBe(0);
    expect(compareVersionPrecedence("not-a-version", "1.0.0")).toBeNull();
  });

  it("leaves compareSemver's MAJOR.MINOR.PATCH comparison alone for its other callers", () => {
    expect(compareSemver("5.0.0-rc.1", "5.0.0-rc.2")).toBe(0);
  });

  it("reaches a 5.0.0-rc.1 install through the tool, at GHSA-82fw's medium tier", () => {
    const db = createDb();
    declare(db, "d:/next", "vitest", "javascript", "5.0.0-rc.1");
    for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
    link(db, addItem(db, VITEST_CVE_ROW), "vitest");
    expect(gapFor(db, "vitest")?.gap_severity).toBe("high");
  });

  it("drops the row once the install reaches the fixing prerelease", () => {
    const db = createDb();
    declare(db, "d:/next", "vitest", "javascript", "5.0.0-rc.2");
    for (const a of VITEST_ADVISORIES) storeAdvisory(db, a);
    link(db, addItem(db, VITEST_CVE_ROW), "vitest");
    expect(gapFor(db, "vitest")).toBeUndefined();
  });
});

describe("(h) ecosystems never mix", () => {
  it("a crates.io advisory never reaches the npm package of the same name", () => {
    const db = createDb();
    declare(db, "d:/web", "jsonwebtoken", "javascript", "9.3.1");
    storeAdvisory(db, H395);
    link(db, addItem(db, H395_ROW), "jsonwebtoken");
    expect(gapFor(db, "jsonwebtoken")).toBeUndefined();
  });
});

describe("versionInAnyRange mirrors osv/matching.rs::check_version_affected", () => {
  const ranges = (json: string) => parseAffectedRanges(json) ?? [];

  it("reads last_affected as an inclusive upper bound (test_last_affected)", () => {
    const r = ranges('[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"last_affected":"1.5.0"}]}]');
    expect(versionInAnyRange(r, "1.3.0")).toBe(true);
    expect(versionInAnyRange(r, "1.5.0")).toBe(true);
    expect(versionInAnyRange(r, "1.5.1")).toBe(false);
  });

  it("holds several pairs in one range (test_version_in_compound_range)", () => {
    const r = ranges('[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"fixed":"1.0.5"},{"introduced":"2.0.0"},{"fixed":"2.1.0"}]}]');
    expect(versionInAnyRange(r, "1.0.3")).toBe(true);
    expect(versionInAnyRange(r, "1.5.0")).toBe(false);
    expect(versionInAnyRange(r, "2.0.5")).toBe(true);
    expect(versionInAnyRange(r, "2.1.0")).toBe(false);
  });

  it("closes a pair at a limit exclusively, like fixed; '*' is no limit", () => {
    const r = ranges('[{"type":"ECOSYSTEM","events":[{"introduced":"1.0.0"},{"limit":"2.0.0"}]}]');
    expect(versionInAnyRange(r, "1.9.9")).toBe(true);
    expect(versionInAnyRange(r, "2.0.0")).toBe(false);
    expect(versionInAnyRange(ranges('[{"events":[{"introduced":"1.0.0"},{"limit":"*"}]}]'), "9.0.0")).toBe(true);
  });

  it("opens a range from an introduced only when nothing closes it (test_introduced_no_fixed)", () => {
    const r = ranges('[{"type":"SEMVER","events":[{"introduced":"1.0.0"},{"fixed":"1.2.0"},{"introduced":"2.0.0"}]}]');
    expect(versionInAnyRange(r, "1.5.0")).toBe(false);
    expect(versionInAnyRange(r, "1.9.0")).toBe(false);
    expect(versionInAnyRange(r, "2.5.0")).toBe(true);
  });

  it("never matches on an unknown bound (test_na_unknown_boundary_does_not_match)", () => {
    const na = ranges('[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"last_affected":"2.5.0-NA"},{"last_affected":"2.7.1-NA"}]}]');
    expect(versionInAnyRange(na, "2.3.0")).toBe(false);
    // A concrete prerelease bound still compares.
    expect(versionInAnyRange(ranges('[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"2.7.1-rc1"}]}]'), "2.5.0")).toBe(true);
  });

  it("never matches on a bound it cannot read, and leaves no open range behind", () => {
    expect(versionInAnyRange(ranges('[{"events":[{"introduced":"0"},{"fixed":"banana"}]}]'), "1.0.0")).toBe(false);
  });

  it("reads v-prefixed and two-part versions as the Rust parser does", () => {
    const r = ranges('[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"2.0.0"}]}]');
    expect(versionInAnyRange(r, "v1.5.0")).toBe(true);
    expect(versionInAnyRange(r, "1.5")).toBe(true);
  });

  it("admits a prerelease install above RUSTSEC's 0.0.0-0 lower bound", () => {
    // RUSTSEC-2023-0071 (rsa, the Marvin attack) has no fix: every version is inside.
    expect(versionInAnyRange(ranges('[{"type":"SEMVER","events":[{"introduced":"0.0.0-0"}]}]'), "0.10.0-rc.18")).toBe(true);
  });

  it("reads only SEMVER and ECOSYSTEM ranges; [] is readable and empty; bad JSON is unreadable", () => {
    expect(parseAffectedRanges('[{"type":"GIT","events":[{"introduced":"abc123"},{"fixed":"def456"}]}]')).toEqual([]);
    expect(parseAffectedRanges("[]")).toEqual([]);
    expect(parseAffectedRanges("not json")).toBeNull();
    expect(parseAffectedRanges(null)).toBeNull();
  });
});

describe("exposure is judged advisory by advisory", () => {
  /** Constructed: an osv row whose advisory the mirror has not synced. */
  const UNSYNCED_ROW: ItemFixture = {
    title: "[GHSA-zzzz-zzzz-zzzz] hono: an advisory the mirror has not synced yet",
    source_type: "osv",
    source_id: "GHSA-zzzz-zzzz-zzzz",
    content_type: "security_advisory",
  };

  it("one unreadable advisory row no longer turns a patched package vulnerable", () => {
    const db = createDb();
    declare(db, "d:/4da/mcp-4da-server", "hono", "javascript", "4.13.5");
    storeAdvisory(db, { id: "GHSA-crvj-82cr-hjcx", pkg: "hono", ecosystem: "npm", ranges: FIXED_AT("4.13.5"), cvss: 5.9, label: "medium", aliases: ["CVE-2026-84363"] });
    storeAdvisory(db, { id: "GHSA-unreadable-range", pkg: "hono", ecosystem: "npm", ranges: "not json" });
    storeAdvisory(db, { id: "GHSA-missing-range", pkg: "hono", ecosystem: "npm", ranges: null });
    link(db, addItem(db, UNSYNCED_ROW), "hono");
    // The row resolves to nothing, so the package decides: every readable range excludes 4.13.5.
    expect(gapFor(db, "hono")?.gap_severity).toBe("low");
  });

  it("an install nothing readable speaks for stays conservatively exposed", () => {
    const db = createDb();
    declare(db, "d:/4da/mcp-4da-server", "hono", "javascript", "4.13.5");
    storeAdvisory(db, { id: "GHSA-unreadable-range", pkg: "hono", ecosystem: "npm", ranges: "not json" });
    link(db, addItem(db, UNSYNCED_ROW), "hono");
    expect(gapFor(db, "hono")?.gap_severity).toBe("high");
  });
});

describe("tiers come from the advisory, not a keyword", () => {
  it("reads the CVSS band first, then the curated label", () => {
    expect(advisoryTier(9.1, "medium")).toBe("critical");
    expect(advisoryTier(7.0, null)).toBe("high");
    expect(advisoryTier(4.0, "critical")).toBe("medium");
    expect(advisoryTier(3.9, null)).toBe("low");
    expect(advisoryTier(null, "high")).toBe("high");
    expect(advisoryTier(null, "moderate")).toBeNull();
    expect(advisoryTier(null, null)).toBeNull();
  });

  it("grades a still-reaching advisory critical only at a critical or high tier", () => {
    const advisory = [{ title: "[GHSA-f23p-vx2j-j53r] hono: memo() retains SSR output across requests", source_type: "osv" }];
    expect(gradeGap(advisory, "hono", true, null, "critical")).toBe("critical");
    expect(gradeGap(advisory, "hono", true, null, "high")).toBe("critical");
    expect(gradeGap(advisory, "hono", true, null, "medium")).toBe("high");
    expect(gradeGap(advisory, "hono", true, null, "low")).toBe("high");
    expect(gradeGap(advisory, "hono", true, null, null)).toBe("high");
    expect(gradeGap(advisory, "hono", false, null, "critical")).toBe("low");
  });

  it("grades a breaking change, deprecation or end of life high, and a release medium", () => {
    expect(gradeGap([{ title: "axum 0.9 breaking changes", source_type: "rss" }], "axum")).toBe("high");
    expect(gradeGap([{ title: "The serde_yaml crate is deprecated", source_type: "rss" }], "serde_yaml")).toBe("high");
    expect(gradeGap([{ title: "Node 18 reaches end-of-life, and so does node-sass", source_type: "rss" }], "node-sass")).toBe("high");
    expect(gradeGap([{ title: "Announcing axum 0.8.0", source_type: "rss" }], "axum")).toBe("medium");
  });
});

describe("a gap names every exposed project, each judged on its own install", () => {
  /** Constructed: one crates.io advisory fixed at 0.8.0, CVSS 7.5. */
  const AXUM_ADVISORY: AdvisoryFixture = { id: "GHSA-test-axum", pkg: "axum", ecosystem: "crates.io", ranges: FIXED_AT("0.8.0"), cvss: 7.5 };
  const AXUM_ROW: ItemFixture = {
    title: "[GHSA-test-axum] axum: request smuggling in the HTTP/1 path",
    source_type: "osv",
    source_id: "GHSA-test-axum",
    content_type: "security_advisory",
  };

  it("names the exposed projects, led by the first, with that project's own version", () => {
    const db = createDb();
    declare(db, "d:/4da/relay", "axum", "rust", "0.7.9");
    declare(db, "d:/4da/src-tauri", "axum", "rust", "0.8.9");
    declare(db, "d:/4da/victauri-gauntlet", "axum", "rust", "0.7.5");
    storeAdvisory(db, AXUM_ADVISORY);
    link(db, addItem(db, AXUM_ROW), "axum");
    const gap = gapFor(db, "axum");
    expect(gap?.gap_severity).toBe("critical");
    expect(gap?.project_path).toBe("d:/4da/relay (+1 more)");
    expect(gap?.version).toBe("0.7.9");
  });

  it("resolves a workspace member's install from the workspace lockfile under the same active root", () => {
    const db = createDb();
    commit(db, "D:\\4DA", 1);
    declare(db, "d:/4da/src-tauri/fourda-macros", "axum", "rust");
    db.getRawDb()
      .prepare("INSERT INTO user_dependencies (project_path, package_name, version, ecosystem) VALUES ('d:/4da/src-tauri', 'axum', '0.7.9', 'rust')")
      .run();
    storeAdvisory(db, AXUM_ADVISORY);
    link(db, addItem(db, AXUM_ROW), "axum");
    expect(gapFor(db, "axum")?.version).toBe("0.7.9");
  });

  it("never borrows another project's version when no active root is shared", () => {
    const db = createDb();
    declare(db, "d:/other/service", "axum", "rust");
    db.getRawDb()
      .prepare("INSERT INTO user_dependencies (project_path, package_name, version, ecosystem) VALUES ('d:/4da/src-tauri', 'axum', '0.7.9', 'rust')")
      .run();
    storeAdvisory(db, AXUM_ADVISORY);
    link(db, addItem(db, AXUM_ROW), "axum");
    const gap = gapFor(db, "axum");
    expect(gap?.version).toBeNull();
    // An unknown install stays conservatively exposed; no reaching advisory can be graded.
    expect(gap?.gap_severity).toBe("high");
  });

  it("falls back to the live resolver's resolution for the project when there is no lockfile table", () => {
    const db = createDb();
    db.getRawDb().exec("DROP TABLE user_dependencies");
    declare(db, "d:/4da/relay", "axum", "rust");
    storeAdvisory(db, AXUM_ADVISORY);
    link(db, addItem(db, AXUM_ROW), "axum");
    const resolved = [
      { name: "axum", version: "0.7.9", ecosystem: "crates.io", sourceDirs: ["D:\\4DA\\relay"] },
    ] as unknown as ResolvedDependency[];
    const liveIntel = { isInitialized: () => true, getResolvedDeps: () => resolved };
    const gap = executeKnowledgeGaps(db, { min_severity: "low" }, liveIntel).gaps?.find((g) => g.dependency === "axum");
    expect(gap?.version).toBe("0.7.9");
    expect(gap?.gap_severity).toBe("critical");
  });
});
