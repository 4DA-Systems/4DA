// SPDX-License-Identifier: Apache-2.0
/**
 * upgrade_impact: call-site scanning (TS + Rust, on a temp dir), changelog
 * cross-referencing against the symbols found, report shaping, and the full
 * analysis over an injected fake registry/OSV — offline, no network.
 *
 * The privacy contract is asserted, not assumed: `_meta.sources` must list
 * exactly the hosts the fake fetch saw, and none may be outside the
 * package's registry and OSV.
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { gzipSync } from "node:zlib";
import { matchSymbols, scanCallSites, scanNpmSource, scanRustSource } from "../tools/upgrade-impact-callsites.js";
import { majorsCrossed, nearestVersions, shapeChangelog, upgradeType } from "../tools/upgrade-impact-report.js";
import { analyzeUpgradeImpact, type InstalledDep, type UpgradeImpactParams } from "../tools/upgrade-impact.js";
import { createUpgradeNet, releaseNotesUrl, type UpgradeCache } from "../live/upgrade-sources.js";
import { parseChangelog } from "../live/changelog.js";
import type { FetchFn } from "../live/package-archive.js";

// ---------------------------------------------------------------- helpers

function tarEntry(name: string, body: string): Buffer {
  const data = Buffer.from(body, "utf8");
  const h = Buffer.alloc(512);
  h.write(name, 0, "utf8");
  h.write(data.length.toString(8).padStart(11, "0") + "\0", 124, "ascii");
  h.write("0", 156, "ascii");
  h.write("ustar\0", 257, "ascii");
  h.fill(0x20, 148, 156);
  let sum = 0;
  for (const b of h) sum += b;
  h.write(sum.toString(8).padStart(6, "0") + "\0 ", 148, "ascii");
  const padded = Buffer.alloc(Math.ceil(data.length / 512) * 512);
  data.copy(padded);
  return Buffer.concat([h, padded]);
}

const tgz = (files: Record<string, string>): Buffer =>
  gzipSync(Buffer.concat([...Object.entries(files).map(([n, b]) => tarEntry(n, b)), Buffer.alloc(1024)]));

const CHANGELOG = [
  "# Changelog",
  "## [2.0.0] - 2025-02-01",
  "### Breaking Changes",
  "- `parseConfig` was renamed to `loadConfig`",
  "- Dropped support for Node 18",
  "### Deprecated",
  "- `legacyMode` option",
  "### Fixed",
  "- one", "- two", "- three", "- four", "- five",
  "## [1.5.0] - 2025-01-01",
  "- `Widget` no longer accepts a string",
  "- Fix CVE-2025-0001 in the tokenizer",
  "## [1.0.0] - 2024-06-01",
  "- initial",
].join("\n");

interface FakeRegistry {
  calls: string[];
  fetch: FetchFn;
}

function fakeNpm(options: { changelog?: boolean; osvFail?: boolean } = {}): FakeRegistry {
  const calls: string[] = [];
  const versions = ["1.0.0", "1.4.0", "1.5.0", "2.0.0-beta.1", "2.0.0", "3.0.0-rc.1"];
  const packument = {
    name: "demo-lib",
    repository: { type: "git", url: "git+https://github.com/acme/demo-lib.git" },
    "dist-tags": { latest: "2.0.0" },
    time: Object.fromEntries(versions.map((v, i) => [v, `2025-0${i + 1}-01T00:00:00.000Z`])),
    versions: Object.fromEntries(
      versions.map((v) => [
        v,
        {
          dist: { tarball: `https://registry.npmjs.org/demo-lib/-/demo-lib-${v}.tgz` },
          ...(v === "1.4.0" ? { deprecated: "use 1.5.0" } : {}),
        },
      ]),
    ),
  };
  const archive = tgz(
    options.changelog === false
      ? { "package/README.md": "hi", "package/index.js": "" }
      : { "package/CHANGELOG.md": CHANGELOG, "package/package.json": "{}" },
  );
  const fetch: FetchFn = async (url, init) => {
    calls.push(url);
    if (url === "https://registry.npmjs.org/demo-lib") return Response.json(packument);
    if (url.startsWith("https://registry.npmjs.org/demo-lib/-/")) return new Response(archive);
    if (url === "https://api.osv.dev/v1/query") {
      if (options.osvFail) throw new Error("ECONNRESET");
      const body = JSON.parse(String(init?.body)) as { version: string };
      return Response.json(body.version === "1.0.0" ? { vulns: [{ id: "GHSA-old1" }, { id: "GHSA-both" }] } : { vulns: [{ id: "GHSA-both" }] });
    }
    return new Response("not found", { status: 404 });
  };
  return { calls, fetch };
}

let root: string;

beforeAll(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), "upgrade-impact-"));
  fs.mkdirSync(path.join(root, "src", "nested"), { recursive: true });
  fs.mkdirSync(path.join(root, "node_modules", "demo-lib"), { recursive: true });
  fs.mkdirSync(path.join(root, "crates", "core", "src"), { recursive: true });
  fs.writeFileSync(
    path.join(root, "src", "app.ts"),
    [
      'import { parseConfig, Widget as W, type Options } from "demo-lib";',
      'import DemoDefault from "demo-lib";',
      'import * as demoNs from "demo-lib/sub";',
      'import other from "demo-lib-extra";',
      'export { helper } from "demo-lib";',
      "const lazy = await import('demo-lib');",
    ].join("\n"),
  );
  fs.writeFileSync(path.join(root, "src", "nested", "legacy.cjs"), "const { legacyMode, run: go } = require('demo-lib');\n");
  fs.writeFileSync(path.join(root, "node_modules", "demo-lib", "index.js"), 'import { ignored } from "demo-lib";\n');
  fs.writeFileSync(
    path.join(root, "crates", "core", "src", "lib.rs"),
    [
      "use fast_embed::{TextEmbedding, InitOptions as Opts, models::{EmbeddingModel, self}};",
      "use fast_embed::Pooling;",
      "extern crate fast_embed;",
      "fn f() { let m = fast_embed::TextEmbedding::try_new(); crate::fast_embed::Local::x(); }",
    ].join("\n"),
  );
});

afterAll(() => {
  fs.rmSync(root, { recursive: true, force: true });
});

// ---------------------------------------------------------------- call sites

describe("call-site scanning", () => {
  it("collects named, default and namespace imports, require destructuring and dynamic import", () => {
    const symbols = new Set<string>();
    const src = fs.readFileSync(path.join(root, "src", "app.ts"), "utf8");
    expect(scanNpmSource(src, "demo-lib", symbols)).toBe(5); // demo-lib-extra is a different package
    expect([...symbols].sort()).toEqual(["DemoDefault", "Options", "Widget", "demoNs", "helper", "parseConfig"]);
  });

  it("collects Rust use-tree leaves and qualified paths, but not crate:: local modules", () => {
    const symbols = new Set<string>();
    const src = fs.readFileSync(path.join(root, "crates", "core", "src", "lib.rs"), "utf8");
    expect(scanRustSource(src, "fast-embed", symbols)).toBe(4);
    expect([...symbols].sort()).toEqual(["EmbeddingModel", "InitOptions", "Pooling", "TextEmbedding", "try_new"]);
  });

  it("walks the project, skipping node_modules, with relative POSIX paths", async () => {
    const report = await scanCallSites(root, "demo-lib", "npm");
    expect(report.total_files).toBe(2);
    expect(report.files.map((f) => f.path)).toEqual(["src/app.ts", "src/nested/legacy.cjs"]);
    expect(report.symbols_used).toContain("legacyMode");
    expect(report.symbols_used).not.toContain("ignored");
  });

  it("scans only .rs files for crates", async () => {
    const report = await scanCallSites(root, "fast_embed", "crates.io");
    expect(report.files).toEqual([{ path: "crates/core/src/lib.rs", matches: 4 }]);
  });
});

// ---------------------------------------------------------------- cross-reference

describe("cross-referencing", () => {
  it("matches identifiers at word boundaries, case-sensitively, ignoring names under 3 chars", () => {
    expect(matchSymbols("`parseConfig` was renamed", ["parseConfig", "Config", "fs"])).toEqual(["parseConfig"]);
    expect(matchSymbols("the widget changed", ["Widget"])).toEqual([]);
  });

  it("matches a plain lowercase word only when the entry marks it as code", () => {
    expect(matchSymbols("Add new model", ["new"])).toEqual([]);
    expect(matchSymbols("`new` now returns Result", ["new"])).toEqual(["new"]);
    expect(matchSymbols("TextEmbedding::new is fallible", ["new"])).toEqual(["new"]);
  });

  it("flags breaking entries touching used symbols, puts them first, and trims concise output", () => {
    const sections = parseChangelog(CHANGELOG).slice(0, 2);
    const concise = shapeChangelog(sections, ["loadConfig", "Widget"], "concise");
    expect(concise.breaking).toBe(3);
    expect(concise.touching).toBe(2); // breaking entries only
    expect(concise.touchingSymbols).toEqual(["Widget", "loadConfig"]);
    expect(concise.sections[0].entries[0]).toMatchObject({ kind: "breaking", touches_your_code: true, matched_symbols: ["loadConfig"] });
    expect(concise.sections[0].entries.filter((e) => e.kind === "change")).toHaveLength(3);
    expect(concise.sections[0].omitted_changes).toBe(2);

    const detailed = shapeChangelog(sections, [], "detailed");
    expect(detailed.sections[0].entries).toHaveLength(8);
    expect(detailed.sections[0].omitted_changes).toBeUndefined();
  });

  it("lists a change repeated across a release and its prereleases once, in the newest section", () => {
    const shaped = shapeChangelog(
      [
        { version: "0.8.0", date: null, entries: [{ kind: "breaking", text: "**breaking:** Remove `Foo` ([#3088])" }] },
        { version: "0.8.0-rc.1", date: null, entries: [{ kind: "breaking", text: "**breaking:** Remove `Foo` (#3088)" }, { kind: "change", text: "only here" }] },
      ],
      [],
      "detailed",
    );
    expect(shaped.breaking).toBe(1);
    expect(shaped.duplicates).toBe(1);
    expect(shaped.sections[1].entries.map((e) => e.text)).toEqual(["only here"]);
  });

  it("caps detailed output at 400 entries and says so", () => {
    const big = [{ version: "2.0.0", date: null, entries: Array.from({ length: 450 }, (_, i) => ({ kind: "change" as const, text: `e${i}` })) }];
    const shaped = shapeChangelog(big, [], "detailed");
    expect(shaped.sections[0].entries).toHaveLength(400);
    expect(shaped.truncated).toMatch(/50 later entries/);
  });
});

describe("report helpers", () => {
  it("classifies upgrade types, treating 0.x minors as major", () => {
    expect(upgradeType("1.2.3", "1.2.4")).toBe("patch");
    expect(upgradeType("1.2.3", "1.3.0")).toBe("minor");
    expect(upgradeType("1.2.3", "2.0.0")).toBe("major");
    expect(upgradeType("0.32.0", "0.37.0")).toBe("major");
    expect(upgradeType("1.0.0", "2.0.0-rc.1")).toBe("prerelease");
    expect(upgradeType("abc", "1.0.0")).toBe("unknown");
    expect(majorsCrossed("5.17.4", "7.1.0")).toBe(2);
    expect(majorsCrossed("0.32.0", "0.37.0")).toBe(5);
  });

  it("lists the nearest published versions around a missing one", () => {
    expect(nearestVersions(["1.0.0", "1.1.0", "1.2.0", "2.0.0", "2.1.0", "3.0.0"], "1.9.0")).toEqual([
      "1.1.0", "1.2.0", "2.0.0", "2.1.0", "3.0.0",
    ]);
  });

  it("derives release-notes URLs from registry repository fields only", () => {
    expect(releaseNotesUrl("git+https://github.com/acme/demo.git")).toBe("https://github.com/acme/demo/releases");
    expect(releaseNotesUrl("github:acme/demo")).toBe("https://github.com/acme/demo/releases");
    expect(releaseNotesUrl("https://github.com/Anush008/fastembed-rs")).toBe("https://github.com/Anush008/fastembed-rs/releases");
    expect(releaseNotesUrl("https://gitlab.com/acme/demo.git")).toBe("https://gitlab.com/acme/demo");
    expect(releaseNotesUrl("svn://example.org/x")).toBeNull();
    expect(releaseNotesUrl(null)).toBeNull();
  });
});

// ---------------------------------------------------------------- analysis

function memoryCache(): UpgradeCache & { keys: () => string[] } {
  const store = new Map<string, unknown>();
  return {
    get: <T>(key: string) => (store.has(key) ? (store.get(key) as T) : null),
    set: (key, data) => void store.set(key, JSON.parse(JSON.stringify(data))),
    keys: () => [...store.keys()],
  };
}

async function run(
  params: UpgradeImpactParams,
  registry: FakeRegistry,
  extra: { installed?: InstalledDep[]; offline?: boolean; cache?: UpgradeCache } = {},
): Promise<Record<string, any>> {
  return analyzeUpgradeImpact(params, {
    net: createUpgradeNet(extra.cache ?? null, registry.fetch),
    projectRoot: root,
    installed: extra.installed ?? [{ name: "demo-lib", version: "1.0.0", ecosystem: "npm", isDirect: true }],
    offline: extra.offline ?? false,
  }) as Promise<Record<string, any>>;
}

describe("analyzeUpgradeImpact", () => {
  it("produces the full report from the installed version to the latest stable", async () => {
    const registry = fakeNpm();
    const r = await run({ package: "demo-lib" }, registry);
    expect(r.error).toBeUndefined();
    expect(r).toMatchObject({ package: "demo-lib", ecosystem: "npm", from_version: "1.0.0", to_version: "2.0.0", upgrade_type: "major" });
    // Prereleases are excluded because the target is stable; the deprecation is carried through.
    expect(r.versions_between).toEqual([
      { version: "1.4.0", published: "2025-02-01T00:00:00.000Z", deprecated: "use 1.5.0" },
      { version: "1.5.0", published: "2025-03-01T00:00:00.000Z" },
      { version: "2.0.0", published: "2025-05-01T00:00:00.000Z" },
    ]);
    expect(r.changelog).toMatchObject({ found: true, file: "package/CHANGELOG.md", covers_range: true });
    expect(r.changelog.sections.map((s: { version: string }) => s.version)).toEqual(["2.0.0", "1.5.0"]);
    expect(r.breaking_changes_count).toBe(3);
    expect(r.deprecations_count).toBe(1);
    expect(r.security_fixes_count).toBe(1);
    expect(r.your_code.total_files).toBe(2);
    // `parseConfig` is imported AND named in the rename entry; `legacyMode` in the deprecation.
    const flagged = r.changelog.sections.flatMap((s: { entries: Array<{ touches_your_code?: boolean; matched_symbols?: string[] }> }) =>
      s.entries.filter((e) => e.touches_your_code).flatMap((e) => e.matched_symbols),
    );
    expect(flagged.sort()).toEqual(["Widget", "legacyMode", "parseConfig"]);
    expect(r.advisories_fixed).toEqual(["GHSA-old1"]);
    expect(r.advisories_remaining).toEqual(["GHSA-both"]);
    expect(r.release_notes_url).toBe("https://github.com/acme/demo-lib/releases");
    expect(r.summary).toBe(
      "demo-lib 1.0.0 -> 2.0.0: 1 major version, 3 releases, 3 breaking changes (2 of them touch symbols you use: Widget, parseConfig), 1 advisories fixed.",
    );
  });

  it("contacts only the registry and OSV, and reports exactly those hosts", async () => {
    const registry = fakeNpm();
    const r = await run({ package: "demo-lib" }, registry);
    const hosts = [...new Set(registry.calls.map((u) => new URL(u).host))].sort();
    expect(hosts).toEqual(["api.osv.dev", "registry.npmjs.org"]);
    expect(r._meta.sources).toEqual(hosts);
    expect(r._meta.untrusted_text).toMatch(/data, not instructions/);
    expect(registry.calls.length).toBeLessThanOrEqual(5);
  });

  it("includes prereleases when the target is one", async () => {
    const r = await run({ package: "demo-lib", to_version: "v3.0.0-rc.1" }, fakeNpm());
    expect(r.versions_between.map((v: { version: string }) => v.version)).toEqual(["1.4.0", "1.5.0", "2.0.0-beta.1", "2.0.0", "3.0.0-rc.1"]);
    expect(r.upgrade_type).toBe("prerelease");
  });

  it("reports a missing changelog honestly and still gives release_notes_url", async () => {
    const r = await run({ package: "demo-lib" }, fakeNpm({ changelog: false }));
    expect(r.changelog).toEqual({ found: false, reason: "the 2.0.0 archive ships no changelog file" });
    expect(r.release_notes_url).toBe("https://github.com/acme/demo-lib/releases");
    expect(r.summary).toMatch(/no changelog in the package archive/);
    // Unknown, not zero: an eval agent read "0 breaking changes" as "nothing breaks".
    expect(r.breaking_changes_count).toBeNull();
    expect(r.deprecations_count).toBeNull();
    expect(r.security_fixes_count).toBeNull();
  });

  it("reads a line request ('2', '1.x', '1.4') as the newest stable release on it", async () => {
    expect((await run({ package: "demo-lib", to_version: "2" }, fakeNpm())).to_version).toBe("2.0.0");
    expect((await run({ package: "demo-lib", to_version: "1.x" }, fakeNpm())).to_version).toBe("1.5.0");
    // 1.0.0 is published, and "1" compares equal to it: the line still wins (axum "0.8" read as 0.8.0, not 0.8.4).
    expect((await run({ package: "demo-lib", to_version: "1" }, fakeNpm())).to_version).toBe("1.5.0");
    expect((await run({ package: "demo-lib", to_version: "1.4" }, fakeNpm())).to_version).toBe("1.4.0");
    const missing = await run({ package: "demo-lib", to_version: "9" }, fakeNpm());
    expect(missing.error).toMatch(/not published/);
    // The suggestions are not empty for a line request.
    expect(missing.error).toMatch(/Nearest published versions: .*2\.0\.0/);
  });

  it("names every installed copy when the project has more than one version", async () => {
    const r = await run({ package: "demo-lib" }, fakeNpm(), {
      installed: [
        { name: "demo-lib", version: "1.0.0", ecosystem: "npm", isDirect: true, sourceDirs: [root] },
        { name: "demo-lib", version: "1.4.0", ecosystem: "npm", isDirect: false, sourceDirs: [path.join(root, "tools")] },
      ],
    });
    expect(r.from_version).toBe("1.0.0");
    expect(r.installed_copies).toEqual([
      { version: "1.0.0", direct: true, pinned_in: ["."] },
      { version: "1.4.0", direct: false, pinned_in: ["tools"] },
    ]);
    expect(r.installed_copies_note).toMatch(/from_version is the direct one/);
  });

  it("tolerates OSV failure with null advisory fields and a note", async () => {
    const r = await run({ package: "demo-lib" }, fakeNpm({ osvFail: true }));
    expect(r.advisories_fixed).toBeNull();
    expect(r.advisories_remaining).toBeNull();
    expect(r.advisories_note).toMatch(/could not be reached/);
    expect(r.summary).toMatch(/advisories unknown/);
  });

  it("serves the second call from cache without refetching the registry", async () => {
    const cache = memoryCache();
    const registry = fakeNpm();
    await run({ package: "demo-lib" }, registry, { cache });
    const first = registry.calls.length;
    await run({ package: "demo-lib" }, registry, { cache });
    expect(registry.calls.length).toBe(first);
    expect(cache.keys().some((k) => k.startsWith("upgrade-impact:changelog:npm:demo-lib:2.0.0"))).toBe(true);
  });

  it("returns actionable errors", async () => {
    const registry = fakeNpm();
    expect((await run({ package: "demo-lib", to_version: "1.9.0" }, registry)).error).toMatch(
      /not published on npm\. Nearest published versions: 1\.4\.0, 1\.5\.0, 2\.0\.0-beta\.1, 2\.0\.0, 3\.0\.0-rc\.1/,
    );
    expect((await run({ package: "demo-lib", from_version: "2.0.0", to_version: "1.5.0" }, registry)).error).toMatch(/not older than/);
    expect((await run({ package: "nope-lib", ecosystem: "npm", from_version: "1.0.0" }, registry)).error).toMatch(/not found on npm/);
    expect((await run({ package: "demo-lib" }, registry, { offline: true })).error).toMatch(/FOURDA_OFFLINE/);
    expect((await run({ package: "unknown-dep" }, registry)).error).toMatch(/Pass ecosystem/);
    expect((await run({ package: "" }, registry)).error).toMatch(/required/);
    expect((await run({ package: "../etc", ecosystem: "npm" }, registry)).error).toMatch(/not a valid npm package name/);
    expect((await run({ package: "demo-lib", ecosystem: "npm" }, registry, { installed: [] })).error).toMatch(/Pass from_version/);
  });

  it("requires the ecosystem when the name is a dependency in both registries", async () => {
    const installed: InstalledDep[] = [
      { name: "demo-lib", version: "1.0.0", ecosystem: "npm", isDirect: true },
      { name: "demo_lib", version: "0.1.0", ecosystem: "crates.io", isDirect: true },
    ];
    expect((await run({ package: "demo-lib" }, fakeNpm(), { installed })).error).toMatch(/both npm and crates.io/);
  });

  it("reads crates.io with a User-Agent and the static.crates.io archive", async () => {
    const seen: Array<{ url: string; ua?: string }> = [];
    const fetch: FetchFn = async (url, init) => {
      seen.push({ url, ua: (init?.headers as Record<string, string> | undefined)?.["User-Agent"] });
      if (url === "https://crates.io/api/v1/crates/fast_embed") {
        return Response.json({
          crate: { name: "fast-embed", repository: "https://github.com/acme/fast-embed" },
          versions: [
            { num: "6.0.0", created_at: "2025-03-01T00:00:00Z", yanked: true },
            { num: "5.17.4", created_at: "2025-01-01T00:00:00Z" },
            { num: "7.1.0", created_at: "2025-05-01T00:00:00Z" },
          ],
        });
      }
      if (url === "https://static.crates.io/crates/fast-embed/fast-embed-7.1.0.crate") {
        return new Response(tgz({ "fast-embed-7.1.0/CHANGELOG.md": "## 7.1.0\n- `TextEmbedding::try_new` renamed\n## 6.0.0\n- x\n## 5.17.4\n- y" }));
      }
      if (url === "https://api.osv.dev/v1/query") return Response.json({});
      return new Response("", { status: 404 });
    };
    const r = (await analyzeUpgradeImpact(
      { package: "fast_embed" },
      {
        net: createUpgradeNet(null, fetch),
        projectRoot: root,
        installed: [{ name: "fast-embed", version: "5.17.4", ecosystem: "crates.io", isDirect: true }],
        offline: false,
      },
    )) as Record<string, any>;
    expect(r).toMatchObject({ package: "fast-embed", ecosystem: "crates.io", from_version: "5.17.4", to_version: "7.1.0" });
    expect(r.versions_between).toEqual([
      { version: "6.0.0", published: "2025-03-01T00:00:00Z", yanked: true },
      { version: "7.1.0", published: "2025-05-01T00:00:00Z" },
    ]);
    expect(r.changelog.covers_range).toBe(true);
    expect(r.changelog.sections[0].entries[0]).toMatchObject({ touches_your_code: true, matched_symbols: ["TextEmbedding", "try_new"] });
    expect(seen.filter((s) => s.url.includes("crates.io")).every((s) => s.ua?.includes("4da-mcp-server"))).toBe(true);
  });
});
