// SPDX-License-Identifier: Apache-2.0
/**
 * dependency_check — end-to-end against recorded fixture shapes, no live network.
 *
 * `fetch` is stubbed with a router over minimal fixture JSON (npm full
 * packuments, the crates.io versions API, the crates sparse index, OSV's
 * querybatch + advisory detail). Each case drives the real registry readers,
 * the real OSV scanner and the real verdict logic through a fresh
 * LiveIntelligence on an in-memory database.
 *
 * Also locks the privacy contract: no registry URL ever carries a version.
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach, vi } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import Database from "better-sqlite3";
import { LiveIntelligence } from "../live/index.js";
import { executeDependencyCheck } from "../tools/dependency-check.js";
import { upgradeType } from "../tools/dependency-check-signals.js";
import type { FourDADatabase } from "../db.js";

const fixtureDir = join(dirname(fileURLToPath(import.meta.url)), "fixtures", "dependency-check");
const fixture = (file: string) => JSON.parse(readFileSync(join(fixtureDir, file), "utf8"));
const noDb = null as unknown as FourDADatabase;
const NOW = new Date("2026-04-10T00:00:00Z");

/** OSV advisories by `name@version`. */
type OsvFixture = Record<string, Array<{ id: string; severity: string; summary: string }>>;

interface Router {
  npm: Record<string, unknown>;
  cratesVersions: Record<string, unknown>;
  sparse: Record<string, string>;
  osv: OsvFixture;
  /** Hosts whose requests throw (simulated outage). */
  down: string[];
  requests: string[];
}

let router: Router;
let priorOffline: string | undefined;

function json(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });
}

function route(url: string, init?: RequestInit): Response {
  const u = new URL(url);
  if (router.down.includes(u.hostname)) throw new TypeError("fetch failed (simulated outage)");
  if (u.hostname === "registry.npmjs.org") {
    const name = decodeURIComponent(u.pathname.slice(1));
    const doc = router.npm[name];
    return doc ? json(doc, 200, { etag: `"${name}-v1"` }) : json({ error: "Not found" }, 404);
  }
  if (u.hostname === "crates.io") {
    const m = /^\/api\/v1\/crates\/([^/]+)(\/versions)?$/.exec(u.pathname);
    const doc = m && m[2] ? router.cratesVersions[m[1]] : null;
    return doc ? json(doc) : json({ errors: [{ detail: "Not Found" }] }, 404);
  }
  if (u.hostname === "index.crates.io") {
    const name = u.pathname.split("/").pop()!;
    return router.sparse[name] ? new Response(router.sparse[name]) : new Response("", { status: 404 });
  }
  if (u.hostname === "api.osv.dev" && u.pathname === "/v1/querybatch") {
    const { queries } = JSON.parse(String(init?.body)) as {
      queries: Array<{ package: { name: string }; version: string }>;
    };
    return json({
      results: queries.map((q) => ({
        vulns: (router.osv[`${q.package.name}@${q.version}`] ?? []).map((v) => ({ id: v.id, modified: "2026-04-01T00:00:00Z" })),
      })),
    });
  }
  if (u.hostname === "api.osv.dev" && u.pathname.startsWith("/v1/vulns/")) {
    const id = decodeURIComponent(u.pathname.slice("/v1/vulns/".length));
    const hit = Object.values(router.osv).flat().find((v) => v.id === id);
    return hit
      ? json({ id, summary: hit.summary, database_specific: { severity: hit.severity }, affected: [] })
      : json({}, 404);
  }
  throw new Error(`unrouted request in test: ${url}`);
}

beforeAll(() => {
  priorOffline = process.env.FOURDA_OFFLINE;
  delete process.env.FOURDA_OFFLINE;
});

afterAll(() => {
  if (priorOffline === undefined) delete process.env.FOURDA_OFFLINE;
  else process.env.FOURDA_OFFLINE = priorOffline;
});

beforeEach(() => {
  router = {
    npm: {
      axios: fixture("npm-axios.json"),
      "plain-crypto-js": fixture("npm-plain-crypto-js.json"),
      "steady-lib": fixture("npm-steady-lib.json"),
    },
    cratesVersions: { "fixture-crate": fixture("crates-fixture-crate-versions.json") },
    sparse: {
      "fixture-crate": [
        { name: "fixture-crate", vers: "1.9.0", deps: [{ name: "serde", kind: "normal" }] },
        { name: "fixture-crate", vers: "2.0.0", deps: [{ name: "serde", kind: "normal" }, { name: "proptest", kind: "dev" }] },
        { name: "fixture-crate", vers: "2.0.1", deps: [{ name: "serde", kind: "normal" }, { name: "proptest", kind: "dev" }] },
      ].map((l) => JSON.stringify(l)).join("\n"),
    },
    osv: {},
    down: [],
    requests: [],
  };
  vi.stubGlobal("fetch", vi.fn(async (url: string, init?: RequestInit) => {
    router.requests.push(url);
    return route(url, init);
  }));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function check(items: unknown[]) {
  const live = new LiveIntelligence(new Database(":memory:"));
  return executeDependencyCheck(noDb, { items }, live, { now: NOW });
}

const signal = (r: { signals: Array<{ id: string }> }, id: string) => r.signals.find((s) => s.id === id) as
  | { id: string; value: unknown; evidence: string; effect: string }
  | undefined;

describe("dependency_check verdicts (fixtures)", () => {
  it("(a) axios 1.14.0 -> 1.14.1: trust drop AND a day-old new dependency -> review with both signals", async () => {
    const out = await check([{ ecosystem: "npm", package: "axios", from: "1.14.0", to: "1.14.1" }]);
    const r = out.results[0];
    expect(r.verdict).toBe("review");
    expect(signal(r, "publish_trust")!.effect).toBe("review");
    expect(signal(r, "publish_trust")!.value).toMatchObject({ to: 0, baseline: 2, baseline_version: "1.14.0" });
    const deps = signal(r, "new_dependencies")!;
    expect(deps.effect).toBe("review");
    expect(deps.value).toEqual([expect.objectContaining({ name: "plain-crypto-js", age_days: expect.any(Number) })]);
    expect((deps.value as Array<{ age_days: number }>)[0].age_days).toBeLessThan(2);
    expect(r.reason).toContain("publish_trust");
    expect(r.reason).toContain("new_dependencies");
  });

  it("(b) a normal patch bump, same trust, no scripts, 10 days old -> proceed", async () => {
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.1" }]);
    const r = out.results[0];
    expect(r.verdict).toBe("proceed");
    expect(signal(r, "release_age")!.value).toMatchObject({ days: 10 });
    expect(signal(r, "upgrade_type")!.value).toBe("patch");
    expect(signal(r, "install_script_added")!.effect).toBe("proceed");
    expect(signal(r, "new_dependencies")!.value).toEqual([]);
  });

  it("(c) a 1-day-old release -> wait", async () => {
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.2" }]);
    expect(out.results[0].verdict).toBe("wait");
    expect(out.results[0].reason).toContain("release_age");
  });

  it("(d) a 1-day-old release that fixes an advisory affecting `from` -> proceed, with the security reason", async () => {
    router.osv["steady-lib@1.0.1"] = [{ id: "GHSA-fix0-0001-aaaa", severity: "HIGH", summary: "Prototype pollution" }];
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.1", to: "1.0.2" }]);
    const r = out.results[0];
    expect(r.verdict).toBe("proceed");
    expect(r.reason).toContain("GHSA-fix0-0001-aaaa");
    expect(r.reason).toMatch(/security fix/);
    expect(signal(r, "advisories")!.value).toMatchObject({ affecting_to: [], fixed_by_this_change: ["GHSA-fix0-0001-aaaa"] });
  });

  it("(e) a deprecated target -> avoid", async () => {
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.1", to: "1.0.3" }]);
    expect(out.results[0].verdict).toBe("avoid");
    expect(signal(out.results[0], "yanked_or_deprecated")!.evidence).toContain("Critical bug in 1.0.3");
  });

  it("(f) registry unreachable -> unknown, never proceed", async () => {
    router.down.push("registry.npmjs.org");
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.1" }]);
    expect(out.results[0].verdict).toBe("unknown");
    expect(out.results[0].reason).toMatch(/could not be reached/);
  });

  it("(g) crates.io trustpub_data drop -> review", async () => {
    const out = await check([{ ecosystem: "crates.io", package: "fixture-crate", from: "2.0.0", to: "2.0.1" }]);
    const r = out.results[0];
    expect(r.verdict).toBe("review");
    expect(signal(r, "publish_trust")!.value).toMatchObject({ to: 0, baseline: 2 });
    expect(signal(r, "publish_trust")!.evidence).toContain("fixture-org/fixture-crate");
    expect(signal(r, "new_dependencies")!.value).toEqual([]); // dev-only deps are not runtime
    expect(signal(r, "install_script_added")!.value).toBe("not_applicable");
  });

  it("OSV unreachable turns an otherwise clean item into unknown", async () => {
    router.down.push("api.osv.dev");
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.1" }]);
    expect(out.results[0].verdict).toBe("unknown");
    expect(signal(out.results[0], "advisories")!.effect).toBe("unknown");
  });

  it("a high advisory on the target -> avoid", async () => {
    router.osv["steady-lib@1.0.1"] = [{ id: "GHSA-bad0-0002-bbbb", severity: "HIGH", summary: "RCE" }];
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.1" }]);
    expect(out.results[0].verdict).toBe("avoid");
  });

  it("an unpublished target (listed in `time`, gone from `versions`) -> avoid", async () => {
    const axios = fixture("npm-axios.json");
    delete axios.versions["1.14.1"];
    router.npm.axios = axios;
    const out = await check([{ ecosystem: "npm", package: "axios", from: "1.14.0", to: "1.14.1" }]);
    expect(out.results[0].verdict).toBe("avoid");
    expect(signal(out.results[0], "yanked_or_deprecated")!.evidence).toContain("unpublished");
  });

  it("a package missing from the public registry -> unknown with a typo/hallucination warning", async () => {
    const out = await check([{ ecosystem: "npm", package: "no-such-pkg-xyz", to: "1.0.0" }]);
    expect(out.results[0].verdict).toBe("unknown");
    expect(out.results[0].reason).toMatch(/hallucinated/);
  });

  it("a new dependency is compared against the previous release on its line", async () => {
    const out = await check([{ ecosystem: "npm", package: "axios", to: "1.14.1" }]);
    const r = out.results[0];
    expect(r.verdict).toBe("review");
    expect(signal(r, "publish_trust")!.value).toMatchObject({ baseline_version: "1.14.0", baseline_kind: "previous_on_line" });
    expect(signal(r, "upgrade_type")!.value).toBe("new_dependency");
    expect(signal(r, "install_script_added")!.effect).toBe("info");
  });

  it("privacy: registry requests never carry a version; only OSV sees versions", async () => {
    await check([
      { ecosystem: "npm", package: "axios", from: "1.14.0", to: "1.14.1" },
      { ecosystem: "crates.io", package: "fixture-crate", from: "2.0.0", to: "2.0.1" },
    ]);
    const host = (u: string): string => new URL(u).hostname;
    const registry = router.requests.filter((u) => host(u) !== "api.osv.dev");
    expect(registry.length).toBeGreaterThan(0);
    for (const url of registry) {
      expect(url, url).not.toMatch(/1\.14\.[01]|2\.0\.[01]/);
    }
    // Registry traffic goes only to the documented registry hosts.
    for (const url of registry) {
      expect(["registry.npmjs.org", "crates.io", "index.crates.io"], url).toContain(host(url));
    }
  });

  it("rejects ranges and malformed input instead of guessing", async () => {
    await expect(check([{ ecosystem: "npm", package: "axios", to: "^1.14.0" }])).rejects.toThrow(/exact version/);
    await expect(check([])).rejects.toThrow(/1 to 25/);
    await expect(check([{ ecosystem: "pypi", package: "x", to: "1.0.0" }])).rejects.toThrow(/ecosystem/);
  });

  it("offline mode answers unknown for every item without any request", async () => {
    process.env.FOURDA_OFFLINE = "true";
    try {
      const out = await check([{ ecosystem: "npm", package: "steady-lib", to: "1.0.1" }]);
      expect(out.results[0].verdict).toBe("unknown");
      expect(router.requests).toHaveLength(0);
    } finally {
      delete process.env.FOURDA_OFFLINE;
    }
  });
});

// 2026-10-03 agent eval: 7 of 24 dependency_check trials passed `to: "latest"`
// when asked about adding a package, and were rejected with a retry.
describe('to: "latest"', () => {
  it("resolves to the newest release and says it was asked for; a deprecated latest is still judged (avoid)", async () => {
    const out = await check([{ ecosystem: "npm", package: "steady-lib", to: "Latest" }]);
    const r = out.results[0] as { to: string; to_requested?: string; verdict: string };
    expect(r.to).toBe("1.0.3");
    expect(r.to_requested).toBe("latest");
    expect(r.verdict).toBe("avoid");
  });

  it("works on crates.io too", async () => {
    const out = await check([{ ecosystem: "crates.io", package: "fixture-crate", to: "latest" }]);
    expect(out.results[0]).toMatchObject({ to: "2.0.1", to_requested: "latest" });
  });

  it("an unknown package stays unknown, and `from` is still an exact version", async () => {
    const out = await check([{ ecosystem: "npm", package: "no-such-pkg-xyz", to: "latest" }]);
    expect(out.results[0]).toMatchObject({ verdict: "unknown", to: "latest" });
    await expect(check([{ ecosystem: "npm", package: "steady-lib", from: "latest", to: "1.0.1" }])).rejects.toThrow(/from must be an exact version/);
  });
});

describe("install scripts and packument caching", () => {
  it("a postinstall added in `to` -> review", async () => {
    const lib = fixture("npm-steady-lib.json");
    lib.versions["1.0.1"].scripts = { postinstall: "node collect.js" };
    router.npm["steady-lib"] = lib;
    const out = await check([{ ecosystem: "npm", package: "steady-lib", from: "1.0.0", to: "1.0.1" }]);
    expect(out.results[0].verdict).toBe("review");
    expect(signal(out.results[0], "install_script_added")!.value).toEqual(["postinstall"]);
  });

  it("revalidates a cached packument with If-None-Match and serves the 304", async () => {
    const { NpmPackumentReader } = await import("../live/npm-packument.js");
    const { LiveCache } = await import("../live/cache.js");
    const { RateLimiter } = await import("../live/rate-limiter.js");
    vi.useFakeTimers({ toFake: ["Date"] });
    try {
      vi.setSystemTime(new Date("2026-04-10T00:00:00Z"));
      // Built after the clock is set: the limiter's refill must not see time run backwards.
      const reader = new NpmPackumentReader(new LiveCache(new Database(":memory:")), new RateLimiter({ npm: { maxPerMinute: 100 } }));
      expect(await reader.getReleases("steady-lib")).toMatchObject({ status: "ok" });
      const seen: Array<string | null> = [];
      vi.stubGlobal("fetch", vi.fn(async (_url: string, init?: RequestInit) => {
        seen.push(new Headers(init?.headers).get("if-none-match"));
        return new Response(null, { status: 304 });
      }));
      // Within the freshness window: no request at all.
      await reader.getReleases("steady-lib");
      expect(seen).toHaveLength(0);
      vi.setSystemTime(new Date("2026-04-10T00:20:00Z"));
      const again = await reader.getReleases("steady-lib");
      expect(seen).toEqual(['"steady-lib-v1"']);
      expect(again.status === "ok" && Object.keys(again.data.releases)).toContain("1.0.2");
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("deps prompt", () => {
  it("lists one prompt and renders the workflow in order", async () => {
    const { listPrompts, getPrompt } = await import("../prompts.js");
    expect(listPrompts().map((p) => p.name)).toEqual(["deps"]);
    const text = getPrompt("deps", { scope: "security only" })!.messages[0].content.text;
    // The final step re-runs vulnerability_scan; step 1 may also mention it.
    const order = ["upgrade_planner", "dependency_check", "`proceed`", "upgrade_impact", "tests", "`review`", "vulnerability_scan"].map((k) =>
      k === "vulnerability_scan" ? text.lastIndexOf(k) : text.indexOf(k),
    );
    expect(order.every((i) => i >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(text).toContain("security only");
    expect(getPrompt("nope", undefined)).toBeNull();
  });

  it("routes each app-plan step by its mechanism and treats targets as minimums", async () => {
    const { getPrompt } = await import("../prompts.js");
    const text = getPrompt("deps", undefined)!.messages[0].content.text;
    for (const mechanism of ["manifest_bump", "lockfile_or_parent_update", "mixed", "no_fix"]) {
      expect(text, mechanism).toContain(`\`${mechanism}\``);
    }
    expect(text).toMatch(/target` is a minimum/);
    expect(text).toMatch(/Never add the transitive package as a new direct dependency/);
    expect(text).toMatch(/do not change the policy/);
  });
});

describe("upgrade_type", () => {
  it("classifies bumps, treating a 0.x minor as major", () => {
    expect(upgradeType("1.2.3", "1.2.4")).toBe("patch");
    expect(upgradeType("1.2.3", "1.3.0")).toBe("minor");
    expect(upgradeType("1.2.3", "2.0.0")).toBe("major");
    expect(upgradeType("0.3.1", "0.4.0")).toBe("major");
    expect(upgradeType("0.3.1", "0.3.2")).toBe("patch");
    expect(upgradeType("1.2.3", "1.2.0")).toBe("downgrade");
    expect(upgradeType(undefined, "1.0.0")).toBe("new_dependency");
  });
});
