#!/usr/bin/env node
// npm-changelog plugin for 4DA
// Checks the npm registry for recent releases of packages in the user's tech stack.
// Protocol: reads PluginConfig JSON from stdin, outputs PluginItem[] JSON to stdout.

"use strict";

const https = require("https");

// Well-known language/framework names that aren't npm packages themselves.
// We skip these to avoid false lookups.
const SKIP_TERMS = new Set([
  "javascript", "typescript", "js", "ts", "node", "nodejs",
  "html", "css", "python", "rust", "go", "java", "c", "cpp",
  "c++", "csharp", "c#", "ruby", "php", "swift", "kotlin",
  "dart", "elixir", "haskell", "scala", "perl", "lua",
  "bash", "shell", "sql", "graphql", "wasm", "webassembly",
  "linux", "windows", "macos", "docker", "kubernetes",
]);

/**
 * Fetch JSON from a URL via HTTPS GET.
 * Returns a Promise that resolves with the parsed JSON body.
 */
function fetchJSON(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { "User-Agent": "4DA-npm-changelog/1.0" } }, (res) => {
      if (res.statusCode < 200 || res.statusCode >= 300) {
        // Drain the response so the socket can be reused
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
 * Query the npm registry for the latest version of a package.
 * Returns a PluginItem or null on failure.
 */
async function fetchPackageLatest(name) {
  try {
    // Use the abbreviated metadata endpoint for speed
    const url = `https://registry.npmjs.org/${encodeURIComponent(name)}/latest`;
    const pkg = await fetchJSON(url);

    if (!pkg || !pkg.version) return null;

    // npm doesn't always include a publish date on /latest — fall back to now
    const publishDate = (pkg.time && pkg.time[pkg.version])
      ? pkg.time[pkg.version]
      : new Date().toISOString();

    return {
      title: `${name} v${pkg.version} released`,
      url: `https://www.npmjs.com/package/${name}/v/${pkg.version}`,
      content: pkg.description || `New version ${pkg.version} of ${name}`,
      source_type: "npm_changelog",
      author: null,
      published_at: publishDate,
    };
  } catch (err) {
    process.stderr.write(`npm-changelog: failed to fetch ${name}: ${err.message}\n`);
    return null;
  }
}

/**
 * Determine which tech_stack entries look like npm package names.
 */
function extractPackageNames(techStack) {
  const packages = [];
  for (const item of techStack) {
    const lower = item.toLowerCase().trim();
    if (!lower) continue;
    if (SKIP_TERMS.has(lower)) continue;
    // Accept scoped packages (@scope/name) and simple names
    if (/^@?[a-z0-9][\w.\-]*(?:\/[a-z0-9][\w.\-]*)?$/i.test(lower)) {
      packages.push(lower);
    }
  }
  return packages;
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
    process.stderr.write(`npm-changelog: invalid config JSON: ${e.message}\n`);
    process.stdout.write("[]");
    process.exit(0);
  }

  const techStack = config.tech_stack || [];
  const maxItems = config.max_items || 20;
  const packageNames = extractPackageNames(techStack);

  if (packageNames.length === 0) {
    process.stdout.write("[]");
    process.exit(0);
  }

  // Fetch in parallel, cap at maxItems
  const results = await Promise.all(
    packageNames.slice(0, maxItems).map(fetchPackageLatest)
  );

  const items = results.filter(Boolean).slice(0, maxItems);
  process.stdout.write(JSON.stringify(items));
  process.exit(0);
}

main().catch((err) => {
  process.stderr.write(`npm-changelog: fatal error: ${err.message}\n`);
  process.stdout.write("[]");
  process.exit(0);
});
