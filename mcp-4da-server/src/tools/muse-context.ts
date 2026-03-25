/**
 * 4DA MUSE MCP Tools
 *
 * Creative context engine tools for managing context packs,
 * enriching generation prompts, and querying creative profiles.
 *
 * MUSE is a private parallel development track — these tools are
 * available to Claude/agents but not user-facing in the current app.
 */

import type { FourDADatabase } from "../db.js";

// =============================================================================
// Parameter Types
// =============================================================================

export interface MuseCreatePackParams {
  name: string;
  description?: string;
}

export interface MuseListPacksParams {
  active_only?: boolean;
}

export interface MuseEnrichPromptParams {
  prompt: string;
  pack_id?: string;
}

export interface MuseAddSourcesParams {
  pack_id: string;
  file_paths: string[];
}

export interface MusePackActionParams {
  pack_id: string;
}

export interface MuseActivateParams {
  pack_id: string;
  active: boolean;
}

// =============================================================================
// Tool Definitions
// =============================================================================

export const museCreatePackTool = {
  name: "muse_create_pack",
  description:
    "Create a new MUSE context pack — a named collection of creative signals derived from local files. Packs capture color language, compositional patterns, sonic palette, and thematic tendencies.",
  inputSchema: {
    type: "object" as const,
    properties: {
      name: {
        type: "string",
        description: "Pack name (e.g., 'Meridian Album Art', 'Cyberpunk Stills')",
      },
      description: {
        type: "string",
        description: "Optional description of the pack's purpose or aesthetic direction",
      },
    },
    required: ["name"],
  },
};

export const museListPacksTool = {
  name: "muse_list_packs",
  description:
    "List all MUSE context packs with their status, source count, and confidence. Use active_only=true to see only packs currently influencing generation.",
  inputSchema: {
    type: "object" as const,
    properties: {
      active_only: {
        type: "boolean",
        description: "Only show active packs. Default: false",
        default: false,
      },
    },
  },
};

export const museEnrichPromptTool = {
  name: "muse_enrich_prompt",
  description:
    "Enrich a generation prompt with creative context from the active MUSE pack. Prepends style guidance (color, composition, themes, anti-patterns) to make any AI generation tool context-aware. Works with any text-accepting API (Sora, Runway, Midjourney, DALL-E, Udio, etc.).",
  inputSchema: {
    type: "object" as const,
    properties: {
      prompt: {
        type: "string",
        description: "The original generation prompt to enrich",
      },
      pack_id: {
        type: "string",
        description: "Specific pack to use. If omitted, uses the first active pack.",
      },
    },
    required: ["prompt"],
  },
};

export const museAddSourcesTool = {
  name: "muse_add_sources",
  description:
    "Add source files to a MUSE context pack. Files are classified by type (image, video, audio, project file) and queued for extraction. Unsupported file types are skipped.",
  inputSchema: {
    type: "object" as const,
    properties: {
      pack_id: {
        type: "string",
        description: "ID of the pack to add sources to",
      },
      file_paths: {
        type: "array",
        items: { type: "string" },
        description: "Array of file paths to add (images, videos, audio, project files)",
      },
    },
    required: ["pack_id", "file_paths"],
  },
};

export const museGetStatsTool = {
  name: "muse_stats",
  description:
    "Get MUSE system statistics — total packs, active packs, source file count, generation history count. Quick health check for the creative context engine.",
  inputSchema: {
    type: "object" as const,
    properties: {},
  },
};

export const museActivatePackTool = {
  name: "muse_activate_pack",
  description:
    "Activate or deactivate a MUSE context pack. Active packs influence generation prompts via muse_enrich_prompt.",
  inputSchema: {
    type: "object" as const,
    properties: {
      pack_id: {
        type: "string",
        description: "ID of the pack to activate/deactivate",
      },
      active: {
        type: "boolean",
        description: "true to activate, false to deactivate",
      },
    },
    required: ["pack_id", "active"],
  },
};

// =============================================================================
// Row Types
// =============================================================================

interface PackRow {
  id: string;
  name: string;
  description: string | null;
  pack_type: string;
  is_active: number;
  source_count: number;
  confidence: number;
  thematic_topics: string | null;
  anti_patterns: string | null;
  created_at: string;
  updated_at: string;
}

interface SourceRow {
  id: number;
  pack_id: string;
  file_path: string;
  file_type: string;
  extraction_status: string;
  confidence: number;
  file_hash: string | null;
}

interface StatsRow {
  total_packs: number;
  active_packs: number;
  total_sources: number;
  total_generations: number;
}

// =============================================================================
// Execute Functions
// =============================================================================

export function executeMuseCreatePack(
  db: FourDADatabase,
  params: MuseCreatePackParams,
) {
  const id = crypto.randomUUID();
  const name = params.name;
  const description = params.description ?? null;

  db.getRawDb()
    .prepare(
      `INSERT INTO muse_packs (id, name, description, pack_type, is_active, source_count, confidence)
       VALUES (?, ?, ?, 'custom', 0, 0, 0.0)`,
    )
    .run(id, name, description);

  return {
    id,
    name,
    description,
    pack_type: "custom",
    is_active: false,
    source_count: 0,
    confidence: 0,
    message: `Created MUSE pack "${name}" (${id})`,
  };
}

export function executeMuseListPacks(
  db: FourDADatabase,
  params: MuseListPacksParams,
) {
  const activeOnly = params.active_only ?? false;
  const query = activeOnly
    ? `SELECT id, name, description, pack_type, is_active, source_count, confidence,
              thematic_topics, anti_patterns, created_at, updated_at
       FROM muse_packs WHERE is_active = 1 ORDER BY updated_at DESC`
    : `SELECT id, name, description, pack_type, is_active, source_count, confidence,
              thematic_topics, anti_patterns, created_at, updated_at
       FROM muse_packs ORDER BY updated_at DESC`;

  const rows = db.getRawDb().prepare(query).all() as PackRow[];

  return {
    packs: rows.map((r) => ({
      id: r.id,
      name: r.name,
      description: r.description,
      pack_type: r.pack_type,
      is_active: r.is_active === 1,
      source_count: r.source_count,
      confidence: r.confidence,
      topics: r.thematic_topics ? JSON.parse(r.thematic_topics) : [],
      anti_patterns: r.anti_patterns ? JSON.parse(r.anti_patterns) : [],
      created_at: r.created_at,
      updated_at: r.updated_at,
    })),
    total: rows.length,
  };
}

export function executeMuseEnrichPrompt(
  db: FourDADatabase,
  params: MuseEnrichPromptParams,
) {
  const prompt = params.prompt;

  // Find the pack to use
  let pack: PackRow | undefined;
  if (params.pack_id) {
    pack = db.getRawDb()
      .prepare("SELECT * FROM muse_packs WHERE id = ?")
      .get(params.pack_id) as PackRow | undefined;
  } else {
    pack = db.getRawDb()
      .prepare("SELECT * FROM muse_packs WHERE is_active = 1 ORDER BY updated_at DESC LIMIT 1")
      .get() as PackRow | undefined;
  }

  if (!pack) {
    return {
      original_prompt: prompt,
      enriched_prompt: prompt,
      pack_used: null,
      enrichment_applied: false,
      message: "No active MUSE pack found — returning original prompt",
    };
  }

  // Build enrichment from pack data
  const parts: string[] = [];

  // Parse topics and anti-patterns
  const topics: Array<{ label: string; weight: number }> = pack.thematic_topics
    ? JSON.parse(pack.thematic_topics)
    : [];
  const antiPatterns: Array<{ label: string; weight: number }> = pack.anti_patterns
    ? JSON.parse(pack.anti_patterns)
    : [];

  if (topics.length > 0) {
    parts.push(`Themes: ${topics.slice(0, 5).map((t) => t.label).join(", ")}`);
  }
  if (antiPatterns.length > 0) {
    parts.push(`Avoid: ${antiPatterns.slice(0, 4).map((a) => a.label).join(", ")}`);
  }

  const enriched =
    parts.length > 0
      ? `Style context: ${parts.join(". ")}.\n\n${prompt}`
      : prompt;

  return {
    original_prompt: prompt,
    enriched_prompt: enriched,
    pack_used: { id: pack.id, name: pack.name, confidence: pack.confidence },
    enrichment_applied: parts.length > 0,
  };
}

export function executeMuseAddSources(
  db: FourDADatabase,
  params: MuseAddSourcesParams,
) {
  const MUSE_EXT_MAP: Record<string, string> = {
    // Image
    png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image",
    svg: "image", tiff: "image", tif: "image", bmp: "image", psd: "image",
    raw: "image", cr2: "image", nef: "image", arw: "image", dng: "image",
    heic: "image", heif: "image", exr: "image",
    // Video
    mp4: "video", mov: "video", avi: "video", mkv: "video", webm: "video",
    flv: "video", wmv: "video", m4v: "video",
    // Audio
    wav: "audio", flac: "audio", mp3: "audio", aiff: "audio", aif: "audio",
    ogg: "audio", m4a: "audio", aac: "audio", wma: "audio", opus: "audio",
    // Project files
    aep: "project_file", prproj: "project_file", drp: "project_file",
    als: "project_file", flp: "project_file", blend: "project_file",
    c4d: "project_file", sketch: "project_file", fig: "project_file",
    xd: "project_file",
  };

  let added = 0;
  let skipped = 0;

  const insert = db.getRawDb().prepare(
    `INSERT INTO muse_pack_sources (pack_id, file_path, file_type, extraction_status)
     VALUES (?, ?, ?, 'pending')`,
  );

  for (const filePath of params.file_paths) {
    const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
    const mediaType = MUSE_EXT_MAP[ext];

    if (mediaType) {
      insert.run(params.pack_id, filePath, mediaType);
      added++;
    } else {
      skipped++;
    }
  }

  // Update source count
  db.getRawDb()
    .prepare(
      `UPDATE muse_packs SET source_count = (SELECT COUNT(*) FROM muse_pack_sources WHERE pack_id = ?),
              updated_at = datetime('now') WHERE id = ?`,
    )
    .run(params.pack_id, params.pack_id);

  return {
    added,
    skipped,
    total: params.file_paths.length,
    message: `Added ${added} source(s) to pack (${skipped} skipped — unsupported type)`,
  };
}

export function executeMuseGetStats(db: FourDADatabase) {
  const totalPacks = (
    db.getRawDb().prepare("SELECT COUNT(*) as c FROM muse_packs").get() as { c: number }
  ).c;
  const activePacks = (
    db.getRawDb().prepare("SELECT COUNT(*) as c FROM muse_packs WHERE is_active = 1").get() as {
      c: number;
    }
  ).c;
  const totalSources = (
    db.getRawDb().prepare("SELECT COUNT(*) as c FROM muse_pack_sources").get() as { c: number }
  ).c;
  const totalGenerations = (
    db.getRawDb().prepare("SELECT COUNT(*) as c FROM muse_generations").get() as { c: number }
  ).c;

  // Source breakdown by type
  const typeBreakdown = db.getRawDb()
    .prepare(
      "SELECT file_type, COUNT(*) as count FROM muse_pack_sources GROUP BY file_type ORDER BY count DESC",
    )
    .all() as Array<{ file_type: string; count: number }>;

  return {
    total_packs: totalPacks,
    active_packs: activePacks,
    total_sources: totalSources,
    total_generations: totalGenerations,
    source_breakdown: typeBreakdown,
    status: totalPacks > 0 ? "active" : "no_packs",
  };
}

export function executeMuseActivatePack(
  db: FourDADatabase,
  params: MuseActivateParams,
) {
  db.getRawDb()
    .prepare("UPDATE muse_packs SET is_active = ?, updated_at = datetime('now') WHERE id = ?")
    .run(params.active ? 1 : 0, params.pack_id);

  const pack = db.getRawDb()
    .prepare("SELECT name FROM muse_packs WHERE id = ?")
    .get(params.pack_id) as { name: string } | undefined;

  return {
    pack_id: params.pack_id,
    active: params.active,
    message: pack
      ? `Pack "${pack.name}" ${params.active ? "activated" : "deactivated"}`
      : `Pack ${params.pack_id} not found`,
  };
}
