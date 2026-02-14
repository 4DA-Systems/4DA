#!/usr/bin/env node
/**
 * 4DA CLI — Terminal briefings from your personalized intelligence feed.
 *
 * Usage:
 *   4da briefing            Show today's briefing
 *   4da briefing --hours 48 Look back 48 hours
 *   4da signals             Show actionable signals
 *   4da dna                 Show your Developer DNA
 *   4da stats               Show system stats
 */

import Database from "better-sqlite3";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

// ============================================================================
// ANSI color helpers
// ============================================================================

const c = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  cyan: "\x1b[36m",
  magenta: "\x1b[35m",
};

function bold(s: string) { return `${c.bold}${s}${c.reset}`; }
function dim(s: string) { return `${c.dim}${s}${c.reset}`; }
function red(s: string) { return `${c.red}${s}${c.reset}`; }
function green(s: string) { return `${c.green}${s}${c.reset}`; }
function yellow(s: string) { return `${c.yellow}${s}${c.reset}`; }
function cyan(s: string) { return `${c.cyan}${s}${c.reset}`; }
function magenta(s: string) { return `${c.magenta}${s}${c.reset}`; }

// ============================================================================
// DB Connection
// ============================================================================

function findDb(): string {
  if (process.env.FOURDA_DB_PATH && existsSync(process.env.FOURDA_DB_PATH)) {
    return process.env.FOURDA_DB_PATH;
  }

  const candidates = [
    join(process.cwd(), "data", "4da.db"),
    join(homedir(), ".4da", "4da.db"),
    join(homedir(), "AppData", "Roaming", "com.4da.app", "4da.db"),
    join(homedir(), "Library", "Application Support", "com.4da.app", "4da.db"),
    join(homedir(), ".local", "share", "com.4da.app", "4da.db"),
  ];

  for (const path of candidates) {
    if (existsSync(path)) return path;
  }

  console.error(red("Could not find 4DA database."));
  console.error(dim("Set FOURDA_DB_PATH or run from the 4DA project directory."));
  process.exit(1);
}

function openDb(): Database.Database {
  const path = findDb();
  const db = new Database(path, { readonly: true });
  db.pragma("busy_timeout = 5000");
  return db;
}

// ============================================================================
// Lightweight scoring (mirrors MCP server approach)
// ============================================================================

interface UserContext {
  tech_stack: string[];
  interests: { topic: string; weight: number }[];
  exclusions: string[];
  affinities: { topic: string; weight: number }[];
}

function loadContext(db: Database.Database): UserContext {
  const tech = db.prepare("SELECT technology FROM tech_stack").all() as { technology: string }[];
  const interests = db.prepare("SELECT topic, weight FROM explicit_interests").all() as { topic: string; weight: number }[];
  const exclusions = db.prepare("SELECT topic FROM exclusions").all() as { topic: string }[];
  const affinities = db.prepare("SELECT topic, affinity_score as weight FROM topic_affinities WHERE affinity_score > 0.1 ORDER BY affinity_score DESC LIMIT 30").all() as { topic: string; weight: number }[];

  return {
    tech_stack: tech.map(t => t.technology.toLowerCase()),
    interests: interests.map(i => ({ topic: i.topic.toLowerCase(), weight: i.weight })),
    exclusions: exclusions.map(e => e.topic.toLowerCase()),
    affinities: affinities.map(a => ({ topic: a.topic.toLowerCase(), weight: a.weight })),
  };
}

function scoreItem(title: string, content: string, ctx: UserContext): number {
  const text = (title + " " + content).toLowerCase();
  const words = new Set(text.split(/\W+/).filter(w => w.length > 2));

  // Exclusion check
  for (const ex of ctx.exclusions) {
    if (text.includes(ex)) return 0;
  }

  let score = 0;

  // Interest matching
  for (const interest of ctx.interests) {
    if (text.includes(interest.topic)) {
      score += 0.3 * interest.weight;
    }
  }

  // Tech stack matching
  for (const tech of ctx.tech_stack) {
    if (words.has(tech) || text.includes(tech)) {
      score += 0.2;
    }
  }

  // Learned affinities
  for (const aff of ctx.affinities) {
    if (text.includes(aff.topic)) {
      score += 0.1 * Math.min(aff.weight, 1.0);
    }
  }

  return Math.min(score, 1.0);
}

// ============================================================================
// Source labels
// ============================================================================

const SOURCE_LABELS: Record<string, string> = {
  hackernews: "Hacker News",
  reddit: "Reddit",
  arxiv: "arXiv",
  github: "GitHub",
  producthunt: "Product Hunt",
  youtube: "YouTube",
  twitter: "Twitter/X",
  rss: "RSS",
  devto: "Dev.to",
  lobsters: "Lobsters",
};

// ============================================================================
// Commands
// ============================================================================

interface ScoredItem {
  id: number;
  source_type: string;
  title: string;
  url: string | null;
  score: number;
  created_at: string;
}

function getRelevantItems(db: Database.Database, hours: number, minScore: number, limit: number): ScoredItem[] {
  const cutoff = new Date(Date.now() - hours * 60 * 60 * 1000)
    .toISOString().replace("T", " ").slice(0, 19);

  const ctx = loadContext(db);

  const items = db.prepare(`
    SELECT id, source_type, title, url, content, created_at
    FROM source_items
    WHERE datetime(created_at) >= datetime(?)
    ORDER BY created_at DESC
    LIMIT ?
  `).all(cutoff, limit * 5) as Array<{
    id: number; source_type: string; title: string;
    url: string | null; content: string; created_at: string;
  }>;

  const scored: ScoredItem[] = [];
  for (const item of items) {
    const score = scoreItem(item.title, item.content, ctx);
    if (score >= minScore) {
      scored.push({
        id: item.id,
        source_type: item.source_type,
        title: item.title,
        url: item.url,
        score: Math.round(score * 100) / 100,
        created_at: item.created_at,
      });
    }
  }

  return scored.sort((a, b) => b.score - a.score).slice(0, limit);
}

function briefing(db: Database.Database, hours: number) {
  const items = getRelevantItems(db, hours, 0.2, 30);

  const totalCount = (db.prepare(`
    SELECT COUNT(*) as cnt FROM source_items
    WHERE datetime(created_at) >= datetime(?)
  `).get(
    new Date(Date.now() - hours * 60 * 60 * 1000).toISOString().replace("T", " ").slice(0, 19)
  ) as { cnt: number }).cnt;

  console.log();
  console.log(bold("  4DA Briefing") + dim(` (last ${hours}h)`));
  console.log(dim("  " + "\u2500".repeat(50)));
  console.log();

  if (items.length === 0) {
    console.log(dim("  No relevant items found. Run an analysis in the 4DA app first."));
    console.log();
    return;
  }

  // Stats
  const highCount = items.filter(i => i.score >= 0.5).length;
  const rejection = totalCount > 0
    ? ((1 - items.length / totalCount) * 100).toFixed(1)
    : "0";
  console.log(
    `  ${bold(String(items.length))} relevant` +
    dim(` of ${totalCount} scanned`) +
    ` \u00b7 ${green(String(highCount))} high-signal` +
    ` \u00b7 ${dim(rejection + "% filtered")}`
  );
  console.log();

  // Group by source
  const bySource = new Map<string, ScoredItem[]>();
  for (const item of items) {
    const group = bySource.get(item.source_type) || [];
    group.push(item);
    bySource.set(item.source_type, group);
  }

  for (const [source, sourceItems] of bySource) {
    const label = SOURCE_LABELS[source] || source;
    console.log(`  ${cyan(bold(label))} ${dim(`(${sourceItems.length})`)}`);

    for (const item of sourceItems.slice(0, 5)) {
      const scoreColor = item.score >= 0.5 ? green : item.score >= 0.3 ? yellow : dim;
      const scoreStr = scoreColor((item.score * 100).toFixed(0).padStart(3) + "%");
      const title = item.title.length > 70
        ? item.title.slice(0, 67) + "..."
        : item.title;
      console.log(`    ${scoreStr} ${title}`);
      if (item.url) {
        console.log(`         ${dim(item.url)}`);
      }
    }
    if (sourceItems.length > 5) {
      console.log(dim(`    ... +${sourceItems.length - 5} more`));
    }
    console.log();
  }
}

function signals(db: Database.Database) {
  const items = getRelevantItems(db, 48, 0.4, 15);

  console.log();
  console.log(bold("  4DA Signals") + dim(" (last 48h, score >= 0.4)"));
  console.log(dim("  " + "\u2500".repeat(50)));
  console.log();

  if (items.length === 0) {
    console.log(dim("  No high-signal items found."));
    console.log();
    return;
  }

  for (const item of items) {
    const priority = item.score >= 0.6 ? red("HIGH") : item.score >= 0.4 ? yellow("MED ") : dim("LOW ");
    const title = item.title.length > 65
      ? item.title.slice(0, 62) + "..."
      : item.title;
    console.log(`  ${priority} ${(item.score * 100).toFixed(0)}% ${bold(title)}`);
    if (item.url) {
      console.log(`       ${dim(item.url)}`);
    }
  }
  console.log();
}

function dna(db: Database.Database) {
  console.log();
  console.log(bold("  Developer DNA"));
  console.log(dim("  " + "\u2500".repeat(50)));
  console.log();

  // Primary stack
  const techStack = db.prepare("SELECT technology FROM tech_stack").all() as { technology: string }[];
  if (techStack.length > 0) {
    console.log(`  ${cyan("Stack")}      ${techStack.map(t => bold(t.technology)).join(", ")}`);
  }

  // Detected tech
  const detected = db.prepare(
    "SELECT name, category FROM detected_tech WHERE confidence >= 0.5 ORDER BY confidence DESC LIMIT 10"
  ).all() as { name: string; category: string }[];
  if (detected.length > 0) {
    console.log(`  ${cyan("Detected")}   ${detected.map(t => t.name).join(", ")}`);
  }

  // Interests
  const interests = db.prepare("SELECT topic FROM explicit_interests LIMIT 10").all() as { topic: string }[];
  if (interests.length > 0) {
    console.log(`  ${cyan("Interests")}  ${interests.map(i => i.topic).join(", ")}`);
  }

  // Dependencies
  const depCount = db.prepare(
    "SELECT COUNT(DISTINCT package_name) as cnt FROM project_dependencies WHERE is_dev = 0"
  ).get() as { cnt: number };
  const projectCount = db.prepare(
    "SELECT COUNT(DISTINCT project_path) as cnt FROM project_dependencies"
  ).get() as { cnt: number };
  console.log(`  ${cyan("Projects")}   ${bold(String(projectCount.cnt))} monitored \u00b7 ${bold(String(depCount.cnt))} dependencies`);

  // Affinities
  const affinities = db.prepare(
    "SELECT topic, affinity_score as weight FROM topic_affinities ORDER BY affinity_score DESC LIMIT 8"
  ).all() as { topic: string; weight: number }[];
  if (affinities.length > 0) {
    console.log();
    console.log(`  ${magenta("Learned Affinities")}`);
    const maxWeight = affinities[0]?.weight || 1;
    for (const a of affinities) {
      const barLen = Math.round((a.weight / maxWeight) * 20);
      const bar = green("\u2588".repeat(barLen)) + dim("\u2591".repeat(20 - barLen));
      console.log(`    ${bar} ${a.topic}`);
    }
  }

  // Source volume
  const sources = db.prepare(`
    SELECT source_type, COUNT(*) as total
    FROM source_items GROUP BY source_type ORDER BY total DESC
  `).all() as { source_type: string; total: number }[];
  if (sources.length > 0) {
    console.log();
    console.log(`  ${cyan("Source Volume")}`);
    for (const s of sources) {
      console.log(`    ${dim(s.source_type.padEnd(15))} ${bold(String(s.total).padStart(6))} items`);
    }
  }

  console.log();
}

function stats(db: Database.Database) {
  const total = (db.prepare("SELECT COUNT(*) as cnt FROM source_items").get() as { cnt: number }).cnt;
  const feedback = (db.prepare("SELECT COUNT(*) as cnt FROM feedback").get() as { cnt: number }).cnt;
  const contexts = (db.prepare("SELECT COUNT(*) as cnt FROM context_chunks").get() as { cnt: number }).cnt;
  const deps = (db.prepare("SELECT COUNT(DISTINCT package_name) as cnt FROM project_dependencies WHERE is_dev = 0").get() as { cnt: number }).cnt;

  console.log();
  console.log(bold("  4DA Stats"));
  console.log(dim("  " + "\u2500".repeat(50)));
  console.log();
  console.log(`  Total items:     ${bold(total.toLocaleString())}`);
  console.log(`  Feedback given:  ${bold(feedback.toLocaleString())}`);
  console.log(`  Context chunks:  ${bold(contexts.toLocaleString())}`);
  console.log(`  Dependencies:    ${bold(deps.toLocaleString())}`);
  console.log();
}

// ============================================================================
// Main
// ============================================================================

function showHelp() {
  console.log(`
  ${bold("4DA CLI")} \u2014 The internet searches for you.

  ${bold("Usage:")}
    4da briefing [--hours N]    Today's relevant content (default: 24h)
    4da signals                 High-priority signals (last 48h)
    4da dna                     Your Developer DNA profile
    4da stats                   System statistics

  ${bold("Options:")}
    --hours N                   Lookback period for briefing (default: 24)
    --help                      Show this help

  ${bold("Environment:")}
    FOURDA_DB_PATH              Path to 4DA database (auto-detected if not set)
`);
}

function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  if (!command || command === "--help" || command === "-h") {
    showHelp();
    return;
  }

  const db = openDb();

  try {
    switch (command) {
      case "briefing":
      case "brief": {
        const hoursIdx = args.indexOf("--hours");
        const hours = hoursIdx >= 0 ? parseInt(args[hoursIdx + 1], 10) || 24 : 24;
        briefing(db, hours);
        break;
      }
      case "signals":
      case "signal":
        signals(db);
        break;
      case "dna":
      case "profile":
        dna(db);
        break;
      case "stats":
      case "status":
        stats(db);
        break;
      default:
        console.error(red(`Unknown command: ${command}`));
        showHelp();
        process.exit(1);
    }
  } finally {
    db.close();
  }
}

main();
