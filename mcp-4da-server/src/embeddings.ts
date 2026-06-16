// SPDX-License-Identifier: Apache-2.0
/**
 * Optional, provider-backed semantic embeddings for local knowledge recall.
 *
 * The MCP server ships ZERO embedding dependencies — that is the whole point of
 * the `npx` zero-install promise. Semantic recall therefore lights up ONLY when
 * the user already has an embedding provider reachable:
 *   - Ollama  (local, private, free)  — set settings.embedding.provider = "ollama"
 *   - OpenAI  (cloud, explicit opt-in) — set settings.embedding.provider = "openai"
 *
 * When no provider is configured, the provider is unreachable, the embed model is
 * missing, or FOURDA_OFFLINE is set, every caller falls back to ranked lexical
 * recall and the response says so via `recall_mode`. Critically, the server embeds
 * BOTH the query and the stored rows with the SAME provider/model, so the vector
 * space is always self-consistent — we never mix our vectors with the desktop
 * app's fastembed vectors.
 *
 * No sqlite-vec: similarity is a plain JS cosine over Float32 BLOBs, which is
 * trivially fast for the few-hundred-row local knowledge tables this serves.
 */

import { readFileSync, existsSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { FourDADatabase } from "./db.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// =============================================================================
// Config
// =============================================================================

export interface EmbeddingConfig {
  provider: "ollama" | "openai";
  model: string;
  /** Ollama base URL (ignored for openai). */
  url?: string;
  /** OpenAI API key (ignored for ollama). */
  apiKey?: string;
}

const DEFAULT_OLLAMA_EMBED_MODEL = "nomic-embed-text";
const DEFAULT_OPENAI_EMBED_MODEL = "text-embedding-3-small";
const DEFAULT_OLLAMA_URL = "http://localhost:11434";

/** True when network calls are disabled by the FOURDA_OFFLINE escape hatch. */
export function isOfflineMode(): boolean {
  return String(process.env.FOURDA_OFFLINE || "").toLowerCase() === "true";
}

/**
 * Locate settings.json the same way llm.ts does, so embedding config lives
 * alongside the rest of the user's local configuration.
 */
function findSettingsFile(): string | null {
  const envPath = process.env.FOURDA_SETTINGS_PATH;
  if (envPath && existsSync(envPath)) return envPath;

  const dbPath = process.env.FOURDA_DB_PATH;
  if (dbPath) {
    const sibling = join(dirname(dbPath), "settings.json");
    if (existsSync(sibling)) return sibling;
  }

  const candidates = [
    join(process.cwd(), "data", "settings.json"),
    join(__dirname, "..", "data", "settings.json"),
    join(process.cwd(), "..", "data", "settings.json"),
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

let _cached: { config: EmbeddingConfig | null; path: string | null; mtimeMs: number } | null = null;

/**
 * Resolve the embedding provider. Order of precedence:
 *   1. FOURDA_OFFLINE        -> always null (no network)
 *   2. environment variables -> FOURDA_EMBED_PROVIDER / FOURDA_EMBED_MODEL / ...
 *   3. settings.json         -> the `embedding` block
 *   4. (nothing)             -> null, callers stay lexical
 *
 * Deliberately does NOT auto-derive from the chat `llm` block: enabling semantic
 * recall is an explicit choice so a recall never makes a surprise network call.
 */
export function getEmbeddingConfig(): EmbeddingConfig | null {
  if (isOfflineMode()) return null;

  // 2. Environment override
  const envProvider = (process.env.FOURDA_EMBED_PROVIDER || "").toLowerCase();
  if (envProvider === "ollama" || envProvider === "openai") {
    return normalizeConfig({
      provider: envProvider,
      model: process.env.FOURDA_EMBED_MODEL,
      url: process.env.OLLAMA_URL,
      api_key: process.env.OPENAI_API_KEY,
    });
  }

  // 3. settings.json `embedding` block (mtime-cached)
  const settingsPath = findSettingsFile();
  if (!settingsPath) return null;

  try {
    const mtimeMs = statSync(settingsPath).mtimeMs;
    if (_cached && _cached.path === settingsPath && _cached.mtimeMs === mtimeMs) {
      return _cached.config;
    }

    const settings = JSON.parse(readFileSync(settingsPath, "utf-8")) as {
      embedding?: {
        provider?: string;
        model?: string;
        url?: string;
        base_url?: string;
        api_key?: string;
      };
    };
    const raw = settings.embedding;
    const config = raw
      ? normalizeConfig({
          provider: (raw.provider || "").toLowerCase(),
          model: raw.model,
          url: raw.url || raw.base_url,
          api_key: raw.api_key,
        })
      : null;

    _cached = { config, path: settingsPath, mtimeMs };
    return config;
  } catch {
    return null;
  }
}

function normalizeConfig(raw: {
  provider: string;
  model?: string;
  url?: string;
  api_key?: string;
}): EmbeddingConfig | null {
  if (raw.provider === "ollama") {
    return {
      provider: "ollama",
      model: raw.model || DEFAULT_OLLAMA_EMBED_MODEL,
      url: raw.url || DEFAULT_OLLAMA_URL,
    };
  }
  if (raw.provider === "openai") {
    if (!raw.api_key) return null; // openai needs a key — no key, no semantic
    return {
      provider: "openai",
      model: raw.model || DEFAULT_OPENAI_EMBED_MODEL,
      apiKey: raw.api_key,
    };
  }
  return null;
}

/** A short, stable identity for the active model, stored alongside each vector. */
export function modelTag(config: EmbeddingConfig): string {
  return `${config.provider}:${config.model}`;
}

// =============================================================================
// Embedding calls
// =============================================================================

/** Cap on the text length sent to the embedder, to bound payload size. */
const MAX_EMBED_CHARS = 8000;

/**
 * Embed a single text. Returns null on ANY failure (provider down, model
 * missing, bad response, empty input) so callers degrade to lexical silently.
 */
export async function embedText(
  text: string,
  config: EmbeddingConfig,
  signal?: AbortSignal,
): Promise<Float32Array | null> {
  const input = (text || "").slice(0, MAX_EMBED_CHARS).trim();
  if (!input) return null;

  try {
    if (config.provider === "ollama") {
      const base = config.url || DEFAULT_OLLAMA_URL;
      const res = await fetch(`${base}/api/embeddings`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model: config.model, prompt: input }),
        signal,
      });
      if (!res.ok) return null;
      const data = (await res.json()) as { embedding?: number[] };
      return data.embedding && data.embedding.length
        ? Float32Array.from(data.embedding)
        : null;
    }

    // openai
    const res = await fetch("https://api.openai.com/v1/embeddings", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${config.apiKey}`,
      },
      body: JSON.stringify({ model: config.model, input }),
      signal,
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { data?: Array<{ embedding: number[] }> };
    const emb = data.data?.[0]?.embedding;
    return emb && emb.length ? Float32Array.from(emb) : null;
  } catch {
    return null;
  }
}

// =============================================================================
// Vector helpers (BLOB <-> Float32Array, cosine)
// =============================================================================

/** Pack a vector into a little-endian Float32 BLOB for SQLite storage. */
export function vectorToBlob(vec: Float32Array): Buffer {
  return Buffer.from(vec.buffer, vec.byteOffset, vec.byteLength);
}

/**
 * Unpack a stored BLOB back into a Float32Array. Returns null for empty/
 * misaligned blobs so a corrupt cell never throws mid-recall. Copies the bytes
 * to guarantee 4-byte alignment regardless of the source buffer's offset.
 */
export function blobToVector(blob: Buffer | Uint8Array | null | undefined): Float32Array | null {
  if (!blob || blob.byteLength === 0 || blob.byteLength % 4 !== 0) return null;
  const copy = Buffer.from(blob);
  return new Float32Array(copy.buffer, copy.byteOffset, copy.byteLength / 4);
}

/** Cosine similarity in [-1, 1]; 0 for length mismatch or a zero vector. */
export function cosineSimilarity(a: Float32Array, b: Float32Array): number {
  if (a.length !== b.length || a.length === 0) return 0;
  let dot = 0;
  let na = 0;
  let nb = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    na += a[i] * a[i];
    nb += b[i] * b[i];
  }
  if (na === 0 || nb === 0) return 0;
  return dot / (Math.sqrt(na) * Math.sqrt(nb));
}

// =============================================================================
// Generic semantic scoring over a local knowledge table
// =============================================================================

/** Default cap on rows embedded per call, so a cold cache cannot stall a response. */
const DEFAULT_MAX_NEW_EMBEDS = 48;

export interface SemanticScoreResult {
  /** id -> cosine similarity to the query (only for rows that have a vector). */
  semanticById: Map<number, number>;
  /** False when the query itself could not be embedded — caller stays lexical. */
  queryEmbedded: boolean;
  /** How many candidate rows contributed a semantic score this call. */
  embeddedCount: number;
  /** How many rows were embedded fresh this call (lazy backfill). */
  newlyEmbedded: number;
  /** The provider:model tag the scores were produced with. */
  model: string;
}

/**
 * Score `items` (id + the text to embed) against a query using cached, lazily
 * backfilled embeddings stored in `<table>.embedding` / `<table>.embedding_model`.
 *
 * Shared by every semantic-recall surface (agent_memory, developer_decisions) so
 * the embed/cache/cosine logic lives in exactly one place. `table` is always a
 * server-controlled constant (never user input), so the interpolated SQL is safe.
 *
 * Returns `queryEmbedded: false` (and an empty map) when the provider is
 * unreachable, so callers fall back to pure lexical recall.
 */
export async function semanticScores(
  db: FourDADatabase,
  table: string,
  query: string,
  items: Array<{ id: number; text: string }>,
  config: EmbeddingConfig,
  opts: { maxNewEmbeds?: number } = {},
): Promise<SemanticScoreResult> {
  const tag = modelTag(config);
  const queryVec = await embedText(query, config);
  if (!queryVec) {
    return { semanticById: new Map(), queryEmbedded: false, embeddedCount: 0, newlyEmbedded: 0, model: tag };
  }

  // Cache columns are optional and added lazily for databases that predate them.
  db.ensureColumn(table, "embedding", "BLOB");
  db.ensureColumn(table, "embedding_model", "TEXT");

  const rawDb = db.getRawDb();
  const stored = loadStoredEmbeddings(rawDb, table, items.map((i) => i.id), tag);
  const updateStmt = rawDb.prepare(
    `UPDATE ${table} SET embedding = ?, embedding_model = ? WHERE id = ?`,
  );
  const maxNew = opts.maxNewEmbeds ?? DEFAULT_MAX_NEW_EMBEDS;

  const semanticById = new Map<number, number>();
  let embeddedCount = 0;
  let newlyEmbedded = 0;

  for (const item of items) {
    let vec = stored.get(item.id) ?? null;
    if (!vec && newlyEmbedded < maxNew) {
      vec = await embedText(item.text, config);
      if (vec) {
        try {
          updateStmt.run(vectorToBlob(vec), tag, item.id);
        } catch {
          // Persisting the cache is best-effort; scoring still proceeds.
        }
        newlyEmbedded++;
      }
    }
    if (vec && vec.length === queryVec.length) {
      embeddedCount++;
      semanticById.set(item.id, cosineSimilarity(queryVec, vec));
    }
  }

  return { semanticById, queryEmbedded: true, embeddedCount, newlyEmbedded, model: tag };
}

/**
 * Load cached embeddings for the given ids that were produced by the CURRENT model
 * (a model change invalidates old vectors). Returns a map of id -> vector.
 */
function loadStoredEmbeddings(
  rawDb: ReturnType<FourDADatabase["getRawDb"]>,
  table: string,
  ids: number[],
  modelTagValue: string,
): Map<number, Float32Array> {
  const out = new Map<number, Float32Array>();
  if (ids.length === 0) return out;
  try {
    const placeholders = ids.map(() => "?").join(",");
    const rows = rawDb
      .prepare(
        `SELECT id, embedding, embedding_model FROM ${table} WHERE id IN (${placeholders})`,
      )
      .all(...ids) as Array<{
        id: number;
        embedding: Buffer | null;
        embedding_model: string | null;
      }>;
    for (const row of rows) {
      if (row.embedding && row.embedding_model === modelTagValue) {
        const vec = blobToVector(row.embedding);
        if (vec) out.set(row.id, vec);
      }
    }
  } catch {
    // Defensive: a missing column simply means no cache, so everything re-embeds.
  }
  return out;
}
