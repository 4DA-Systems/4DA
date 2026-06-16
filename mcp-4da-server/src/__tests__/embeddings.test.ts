// SPDX-License-Identifier: Apache-2.0
/**
 * Unit tests for the embedding helpers: cosine, BLOB round-trip, config
 * resolution, and graceful nulls. No network is touched (empty-input embeds
 * return before any fetch; config tests only read env/offline state).
 */

import { describe, it, expect, afterEach } from "vitest";
import {
  cosineSimilarity,
  vectorToBlob,
  blobToVector,
  getEmbeddingConfig,
  isOfflineMode,
  modelTag,
  embedText,
} from "../embeddings.js";

const ENV_KEYS = ["FOURDA_OFFLINE", "FOURDA_EMBED_PROVIDER", "FOURDA_EMBED_MODEL", "OPENAI_API_KEY", "FOURDA_SETTINGS_PATH", "FOURDA_DB_PATH"];

describe("cosineSimilarity", () => {
  it("is 1 for identical vectors", () => {
    const v = Float32Array.from([0.2, 0.5, 0.1, 0.8]);
    expect(cosineSimilarity(v, v)).toBeCloseTo(1, 6);
  });

  it("is 0 for orthogonal vectors and 0 for a zero vector", () => {
    expect(cosineSimilarity(Float32Array.from([1, 0]), Float32Array.from([0, 1]))).toBe(0);
    expect(cosineSimilarity(Float32Array.from([0, 0]), Float32Array.from([1, 1]))).toBe(0);
  });

  it("is -1 for opposite vectors", () => {
    expect(cosineSimilarity(Float32Array.from([1, 1]), Float32Array.from([-1, -1]))).toBeCloseTo(-1, 6);
  });

  it("is 0 for a length mismatch", () => {
    expect(cosineSimilarity(Float32Array.from([1, 2, 3]), Float32Array.from([1, 2]))).toBe(0);
  });
});

describe("BLOB round-trip", () => {
  it("preserves vector values exactly", () => {
    const vec = Float32Array.from([1, -0.5, 3.25, 0, 100.5]);
    const back = blobToVector(vectorToBlob(vec));
    expect(back).not.toBeNull();
    expect(Array.from(back!)).toEqual(Array.from(vec));
  });

  it("returns null for empty/misaligned/absent blobs", () => {
    expect(blobToVector(null)).toBeNull();
    expect(blobToVector(Buffer.alloc(0))).toBeNull();
    expect(blobToVector(Buffer.from([1, 2, 3]))).toBeNull(); // 3 bytes, not a multiple of 4
  });
});

describe("getEmbeddingConfig", () => {
  afterEach(() => {
    for (const k of ENV_KEYS) delete process.env[k];
  });

  it("returns null when offline", () => {
    process.env.FOURDA_OFFLINE = "true";
    process.env.FOURDA_EMBED_PROVIDER = "ollama";
    expect(isOfflineMode()).toBe(true);
    expect(getEmbeddingConfig()).toBeNull();
  });

  it("reads an ollama provider from the environment", () => {
    process.env.FOURDA_EMBED_PROVIDER = "ollama";
    process.env.FOURDA_EMBED_MODEL = "nomic-embed-text";
    const config = getEmbeddingConfig();
    expect(config?.provider).toBe("ollama");
    expect(config?.model).toBe("nomic-embed-text");
    expect(modelTag(config!)).toBe("ollama:nomic-embed-text");
  });

  it("refuses openai without an api key", () => {
    process.env.FOURDA_EMBED_PROVIDER = "openai";
    expect(getEmbeddingConfig()).toBeNull();
  });
});

describe("embedText", () => {
  it("returns null for empty input without making a request", async () => {
    const config = { provider: "ollama" as const, model: "x", url: "http://127.0.0.1:1" };
    expect(await embedText("   ", config)).toBeNull();
  });
});
