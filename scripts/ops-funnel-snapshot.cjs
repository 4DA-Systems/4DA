#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Distribution funnel snapshot — zero-telemetry metrics from public/owned APIs.
 *
 * Appends one JSON line per run to docs/private/distribution/funnel-metrics.jsonl
 * (gitignored). Sources: npm downloads API, GitHub repo traffic/stars (needs gh
 * auth), mcp-v* release asset download counts (.mcpb adoption).
 *
 * Usage: pnpm run ops:funnel   (or node scripts/ops-funnel-snapshot.cjs)
 * Cadence: weekly is plenty; every masterplan metric must inform an action.
 */
const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");

const REPO = "4DA-Systems/4DA";
const OUT = path.join(__dirname, "..", "docs", "private", "distribution", "funnel-metrics.jsonl");

function getJson(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "4da-funnel-snapshot" } }, (res) => {
        let d = "";
        res.on("data", (c) => (d += c));
        res.on("end", () => {
          try { resolve(JSON.parse(d)); } catch (e) { reject(e); }
        });
      })
      .on("error", reject);
  });
}

function gh(args) {
  try {
    return JSON.parse(execSync(`gh api ${args}`, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
  } catch {
    return null; // gh unauthenticated or API hiccup — record null, never fail the run
  }
}

(async () => {
  const [npmWeek, npmMonth] = await Promise.all([
    getJson("https://api.npmjs.org/downloads/point/last-week/@4da/mcp-server").catch(() => null),
    getJson("https://api.npmjs.org/downloads/point/last-month/@4da/mcp-server").catch(() => null),
  ]);

  const repo = gh(`repos/${REPO} --jq "{stars:.stargazers_count,forks:.forks_count,watchers:.subscribers_count}"`);
  const views = gh(`repos/${REPO}/traffic/views --jq "{count:.count,uniques:.uniques}"`);
  const clones = gh(`repos/${REPO}/traffic/clones --jq "{count:.count,uniques:.uniques}"`);

  // .mcpb adoption: download counts across all mcp-v* release assets
  const releases = gh(`repos/${REPO}/releases --jq "[.[] | select(.tag_name | startswith(\\"mcp-v\\")) | {tag:.tag_name, assets:[.assets[] | {name:.name, downloads:.download_count}]}]"`);

  const snapshot = {
    at: new Date().toISOString(),
    npm: {
      week: npmWeek ? npmWeek.downloads : null,
      month: npmMonth ? npmMonth.downloads : null,
    },
    github: { repo, views_14d: views, clones_14d: clones },
    mcpb_releases: releases,
  };

  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.appendFileSync(OUT, JSON.stringify(snapshot) + "\n");

  // Human-readable summary (the metric must inform an action — trends live in the jsonl)
  console.log(`funnel @ ${snapshot.at}`);
  console.log(`  npm @4da/mcp-server: ${snapshot.npm.week ?? "?"}/wk  ${snapshot.npm.month ?? "?"}/mo (mostly mirrors — watch the TREND, not the level)`);
  if (repo) console.log(`  github: ${repo.stars} stars, ${views ? views.uniques : "?"} unique visitors /14d, ${clones ? clones.uniques : "?"} unique cloners /14d`);
  if (releases) {
    for (const r of releases) {
      const total = r.assets.reduce((s, a) => s + (a.downloads || 0), 0);
      console.log(`  ${r.tag}: ${total} .mcpb downloads (${r.assets.map((a) => `${a.name.replace(/^4da-mcp-server-[0-9.]+-/, "").replace(/\.mcpb$/, "")}:${a.downloads}`).join(", ")})`);
    }
  }
  console.log(`  appended -> ${path.relative(process.cwd(), OUT)}`);
})();
