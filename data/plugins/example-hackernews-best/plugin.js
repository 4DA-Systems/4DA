#!/usr/bin/env node
// Example 4DA plugin: Hacker News Best Stories
// Demonstrates the plugin protocol — JSON config on stdin, PluginItem[] on stdout.
// See docs/PLUGIN-DEVELOPMENT.md for the full developer guide.

import https from "node:https";

function fetchJSON(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { "User-Agent": "4DA-plugin/0.1" } }, (res) => {
      if (res.statusCode < 200 || res.statusCode >= 300) {
        res.resume();
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      let data = "";
      res.on("data", (chunk) => { data += chunk; });
      res.on("end", () => {
        try { resolve(JSON.parse(data)); }
        catch (e) { reject(new Error(`Invalid JSON from ${url}`)); }
      });
    });
    req.on("error", reject);
    req.setTimeout(10000, () => req.destroy(new Error("Timeout")));
  });
}

async function main() {
  // 1. Read config from stdin
  let input = "";
  for await (const chunk of process.stdin) input += chunk;

  let config;
  try { config = JSON.parse(input); }
  catch { config = {}; }

  const maxItems = config.max_items || 10;

  // 2. Fetch best story IDs
  const ids = await fetchJSON("https://hacker-news.firebaseio.com/v0/beststories.json");

  // 3. Fetch details for top N stories in parallel
  const details = await Promise.all(
    ids.slice(0, maxItems).map((id) =>
      fetchJSON(`https://hacker-news.firebaseio.com/v0/item/${id}.json`).catch(() => null)
    )
  );

  // 4. Map to PluginItem format and output
  const items = details.filter(Boolean).map((story) => ({
    title: story.title || "Untitled",
    url: story.url || `https://news.ycombinator.com/item?id=${story.id}`,
    content: story.title || "",
    source_type: "hackernews_best",
    author: story.by || null,
    published_at: story.time ? new Date(story.time * 1000).toISOString() : null,
  }));

  process.stdout.write(JSON.stringify(items));
}

main().catch((err) => {
  process.stderr.write(`example-hackernews-best: ${err.message}\n`);
  process.exit(1);
});
