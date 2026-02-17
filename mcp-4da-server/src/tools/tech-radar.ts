/**
 * tech_radar tool
 *
 * Query the user's personal technology radar. Synthesizes data from
 * tech stack, detected technologies, dependencies, decisions, and engagement.
 * Mirrors the Rust tech_radar.rs module using the same tables.
 */

import type { FourDADatabase } from "../db.js";

// ============================================================================
// Types
// ============================================================================

export interface TechRadarParams {
  quadrant?: "languages" | "frameworks" | "tools" | "platforms";
  ring?: "adopt" | "trial" | "assess" | "hold";
  search?: string;
}

interface RadarEntry {
  name: string;
  ring: string;
  quadrant: string;
  source: string; // "primary_stack", "dependency", "detected", "decision_hold"
  decision_ref?: number;
  confidence: number;
}

// ============================================================================
// Quadrant Classification
// ============================================================================

const LANGUAGES = new Set([
  "rust", "typescript", "javascript", "python", "go", "java", "kotlin",
  "swift", "c++", "c#", "ruby", "php", "scala", "elixir", "haskell",
  "zig", "lua", "dart", "r", "julia",
]);

const FRAMEWORKS = new Set([
  "react", "vue", "angular", "svelte", "nextjs", "next.js", "nuxt",
  "remix", "astro", "tauri", "electron", "django", "flask", "fastapi",
  "express", "fastify", "rails", "spring", "actix", "axum", "gin",
  "flutter", "react-native", "sveltekit",
]);

const PLATFORMS = new Set([
  "aws", "gcp", "azure", "vercel", "netlify", "cloudflare", "heroku",
  "docker", "kubernetes", "k8s", "linux", "windows", "macos", "ios",
  "android",
]);

function classifyQuadrant(name: string): string {
  const n = name.toLowerCase();
  if (LANGUAGES.has(n)) return "languages";
  if (FRAMEWORKS.has(n)) return "frameworks";
  if (PLATFORMS.has(n)) return "platforms";
  return "tools";
}

// ============================================================================
// Dependency Filtering
// ============================================================================

const SKIP_DEPS = new Set([
  "proc-macro2", "quote", "unicode-ident", "cfg-if", "memchr", "libc",
  "autocfg", "version_check", "pkg-config", "itoa", "ryu", "bitflags",
  "bytes", "pin-project-lite", "fnv", "percent-encoding", "tinyvec",
  "smallvec", "indexmap", "hashbrown", "equivalent", "either", "anyhow",
  "thiserror",
]);

function isNotableDependency(name: string): boolean {
  if (name.length < 4) return false;
  return !SKIP_DEPS.has(name);
}

// ============================================================================
// Tool Definition
// ============================================================================

export const techRadarTool = {
  name: "tech_radar",
  description: `Query the user's personal technology radar. Synthesizes a ThoughtWorks-style radar from tech stack, dependencies, detected technologies, and developer decisions.

Each entry is placed in a ring:
- adopt: Primary tech stack — actively used daily
- trial: Notable dependencies — in use but not core identity
- assess: Auto-detected technologies — seen in projects but not yet committed
- hold: Rejected alternatives from developer decisions

Entries are classified into quadrants: languages, frameworks, tools, platforms.

Use filters to narrow results by quadrant, ring, or free-text search.`,
  inputSchema: {
    type: "object" as const,
    properties: {
      quadrant: {
        type: "string",
        enum: ["languages", "frameworks", "tools", "platforms"],
        description: "Filter by quadrant. Leave empty for all quadrants.",
      },
      ring: {
        type: "string",
        enum: ["adopt", "trial", "assess", "hold"],
        description: "Filter by ring. Leave empty for all rings.",
      },
      search: {
        type: "string",
        description: "Free-text search to filter entries by name.",
      },
    },
  },
};

// ============================================================================
// Execute
// ============================================================================

export function executeTechRadar(
  db: FourDADatabase,
  params: TechRadarParams,
): object {
  const rawDb = db.getRawDb();
  const entries: RadarEntry[] = [];
  const seen = new Set<string>();

  // 1. Primary tech stack -> ring: "adopt"
  try {
    const rows = rawDb
      .prepare("SELECT technology FROM tech_stack ORDER BY technology")
      .all() as { technology: string }[];

    for (const row of rows) {
      const name = row.technology.toLowerCase();
      if (!seen.has(name)) {
        seen.add(name);
        entries.push({
          name,
          ring: "adopt",
          quadrant: classifyQuadrant(name),
          source: "primary_stack",
          confidence: 1.0,
        });
      }
    }
  } catch {
    // Table may not exist
  }

  // 2. Project dependencies -> ring: "trial"
  try {
    const rows = rawDb
      .prepare(
        "SELECT DISTINCT package_name FROM project_dependencies ORDER BY package_name",
      )
      .all() as { package_name: string }[];

    for (const row of rows) {
      const name = row.package_name.toLowerCase();
      if (!seen.has(name) && isNotableDependency(name)) {
        seen.add(name);
        entries.push({
          name,
          ring: "trial",
          quadrant: classifyQuadrant(name),
          source: "dependency",
          confidence: 0.7,
        });
      }
    }
  } catch {
    // Table may not exist
  }

  // 3. Detected tech -> ring: "assess"
  try {
    const rows = rawDb
      .prepare(
        "SELECT name, confidence FROM detected_tech WHERE confidence > 0.3 ORDER BY confidence DESC",
      )
      .all() as { name: string; confidence: number }[];

    for (const row of rows) {
      const name = row.name.toLowerCase();
      if (!seen.has(name)) {
        seen.add(name);
        entries.push({
          name,
          ring: "assess",
          quadrant: classifyQuadrant(name),
          source: "detected",
          confidence: Math.round(row.confidence * 100) / 100,
        });
      }
    }
  } catch {
    // Table may not exist
  }

  // 4. Developer decisions — rejected alternatives -> ring: "hold"
  try {
    const rows = rawDb
      .prepare(
        `SELECT id, decision_type, subject, alternatives_rejected, status
         FROM developer_decisions
         WHERE status IN ('active', 'superseded') AND decision_type = 'tech_choice'`,
      )
      .all() as {
      id: number;
      decision_type: string;
      subject: string;
      alternatives_rejected: string;
      status: string;
    }[];

    for (const row of rows) {
      let alts: string[] = [];
      try {
        alts = JSON.parse(row.alternatives_rejected);
      } catch {
        // Malformed JSON, skip
      }

      for (const alt of alts) {
        const name = alt.toLowerCase();
        if (!seen.has(name)) {
          seen.add(name);
          entries.push({
            name,
            ring: "hold",
            quadrant: classifyQuadrant(name),
            source: "decision_hold",
            decision_ref: row.id,
            confidence: 0.8,
          });
        }
      }
    }
  } catch {
    // Table may not exist
  }

  // 5. Apply optional filters
  let filtered = entries;

  if (params.quadrant) {
    filtered = filtered.filter((e) => e.quadrant === params.quadrant);
  }

  if (params.ring) {
    filtered = filtered.filter((e) => e.ring === params.ring);
  }

  if (params.search) {
    const term = params.search.toLowerCase();
    filtered = filtered.filter((e) => e.name.includes(term));
  }

  // Sort: adopt > trial > assess > hold, then alphabetically
  const ringOrder: Record<string, number> = {
    adopt: 0,
    trial: 1,
    assess: 2,
    hold: 3,
  };

  filtered.sort((a, b) => {
    const ringDiff = (ringOrder[a.ring] ?? 4) - (ringOrder[b.ring] ?? 4);
    if (ringDiff !== 0) return ringDiff;
    return a.name.localeCompare(b.name);
  });

  return {
    entries: filtered,
    total: filtered.length,
    generated_at: new Date().toISOString(),
    summary: buildSummary(filtered),
  };
}

// ============================================================================
// Helpers
// ============================================================================

function buildSummary(entries: RadarEntry[]): string {
  const byRing: Record<string, number> = {};
  for (const e of entries) {
    byRing[e.ring] = (byRing[e.ring] || 0) + 1;
  }

  const parts: string[] = [];
  if (byRing.adopt) parts.push(`${byRing.adopt} adopt`);
  if (byRing.trial) parts.push(`${byRing.trial} trial`);
  if (byRing.assess) parts.push(`${byRing.assess} assess`);
  if (byRing.hold) parts.push(`${byRing.hold} hold`);

  return parts.length > 0
    ? `${entries.length} technologies: ${parts.join(", ")}`
    : "No technologies found in radar";
}
