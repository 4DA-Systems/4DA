#!/usr/bin/env node
// crates-changelog plugin for 4DA
// Checks crates.io for recent releases of Rust crates in the user's tech stack.
// Protocol: reads PluginConfig JSON from stdin, outputs PluginItem[] JSON to stdout.

import https from "node:https";

// Terms that are language/concept names, not crate names.
// We skip these to avoid false lookups on crates.io.
const SKIP_TERMS = new Set([
  "rust", "cargo", "javascript", "typescript", "python", "go", "java",
  "c", "cpp", "c++", "csharp", "c#", "ruby", "php", "swift", "kotlin",
  "html", "css", "sql", "graphql", "wasm", "webassembly",
  "linux", "windows", "macos", "docker", "kubernetes",
  "node", "nodejs", "js", "ts", "react", "vue", "angular", "svelte",
  "async", "performance", "security", "testing", "cli",
]);

/**
 * Fetch JSON from a URL via HTTPS GET.
 * crates.io requires a User-Agent header per their crawling policy.
 */
function fetchJSON(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, {
      headers: { "User-Agent": "4DA-crates-changelog/1.0 (https://4da.ai)" }
    }, (res) => {
      if (res.statusCode < 200 || res.statusCode >= 300) {
        res.resume();
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      let data = "";
      res.on("data", (chunk) => { data += chunk; });
      res.on("end", () => {
        try {
          resolve(JSON.parse(data));
        } catch (e) {
          reject(new Error(`Invalid JSON from ${url}: ${e.message}`));
        }
      });
    });
    req.on("error", reject);
    req.setTimeout(10000, () => {
      req.destroy(new Error(`Timeout fetching ${url}`));
    });
  });
}

/**
 * Query crates.io for the latest versions of a crate.
 * Returns a PluginItem for the most recent version, or null on failure.
 */
async function fetchCrateLatest(name) {
  try {
    // crates.io API: get crate info including recent versions
    const url = `https://crates.io/api/v1/crates/${encodeURIComponent(name)}`;
    const data = await fetchJSON(url);

    if (!data || !data.crate) return null;

    const crateInfo = data.crate;
    const latestVersion = crateInfo.newest_version || crateInfo.max_version;

    if (!latestVersion) return null;

    // Find the version object for publish date and other metadata
    let publishDate = crateInfo.updated_at || new Date().toISOString();
    let description = crateInfo.description || `Rust crate ${name}`;

    // If versions array is present, get the latest one's details
    if (data.versions && data.versions.length > 0) {
      const latest = data.versions[0]; // versions are sorted newest-first
      if (latest.created_at) publishDate = latest.created_at;
    }

    return {
      title: `${name} v${latestVersion} released`,
      url: `https://crates.io/crates/${name}/${latestVersion}`,
      content: description,
      source_type: "crates_changelog",
      author: null,
      published_at: publishDate,
    };
  } catch (err) {
    process.stderr.write(`crates-changelog: failed to fetch ${name}: ${err.message}\n`);
    return null;
  }
}

/**
 * Extract crate-like names from the tech_stack.
 * Crate names use hyphens or underscores, lowercase alphanumeric.
 */
function extractCrateNames(techStack) {
  const crates = [];
  for (const item of techStack) {
    const lower = item.toLowerCase().trim();
    if (!lower) continue;
    if (SKIP_TERMS.has(lower)) continue;
    // Crate names: alphanumeric, hyphens, underscores
    if (/^[a-z][a-z0-9_\-]*$/i.test(lower)) {
      crates.push(lower);
    }
  }
  return crates;
}

async function main() {
  // Read all stdin
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
  }

  let config;
  try {
    config = JSON.parse(input);
  } catch (e) {
    process.stderr.write(`crates-changelog: invalid config JSON: ${e.message}\n`);
    process.stdout.write("[]");
    process.exit(0);
  }

  const techStack = config.tech_stack || [];
  const maxItems = config.max_items || 20;
  const crateNames = extractCrateNames(techStack);

  if (crateNames.length === 0) {
    process.stdout.write("[]");
    process.exit(0);
  }

  // Fetch sequentially to respect crates.io rate limits (1 req/sec guideline).
  // For a small number of crates this is fine; parallel would risk 429s.
  const items = [];
  for (const name of crateNames.slice(0, maxItems)) {
    const item = await fetchCrateLatest(name);
    if (item) items.push(item);
    // Brief delay between requests to be polite to crates.io
    if (crateNames.length > 1) {
      await new Promise(r => setTimeout(r, 200));
    }
  }

  process.stdout.write(JSON.stringify(items.slice(0, maxItems)));
  process.exit(0);
}

main().catch((err) => {
  process.stderr.write(`crates-changelog: fatal error: ${err.message}\n`);
  process.stdout.write("[]");
  process.exit(0);
});
