// SPDX-License-Identifier: Apache-2.0
/**
 * Unit tests for the shared alias-aware relevance scorer.
 *
 * createRelevanceScorer is the single relevance primitive used by ranked recall
 * (agent_memory, decisions) AND by what_should_i_know's advisory/window/news
 * filtering. These tests pin the behaviour that substring matching lacked:
 * alias expansion (auth -> jwt/oauth/authz), and precompute reuse across texts.
 */

import { describe, it, expect } from "vitest";
import {
  createRelevanceScorer,
  matchesRecallQuery,
  recallTerms,
} from "../tools/recall.js";

describe("createRelevanceScorer", () => {
  it("expands aliases so an 'auth' query matches jwt / oauth / authorization text", () => {
    const score = createRelevanceScorer("auth");

    // The whole point of the fix: substring matching would miss all of these.
    expect(score("Rotate the JWT signing key")).toBeGreaterThan(0);
    expect(score("Migrate to OAuth login")).toBeGreaterThan(0);
    expect(score("Set the Authorization header")).toBeGreaterThan(0);
  });

  it("does not match unrelated text", () => {
    const score = createRelevanceScorer("kubernetes");
    expect(score("Rotate the JWT signing key")).toBe(0);
    expect(score("")).toBe(0);
  });

  it("is reusable: one scorer applied to many texts scores each independently", () => {
    const score = createRelevanceScorer("postgres migration");
    // alias postgres -> postgresql, plus the literal 'migration' term
    expect(score("PostgreSQL migration plan")).toBeGreaterThan(
      score("PostgreSQL connection pool"),
    );
    expect(score("unrelated redis note")).toBe(0);
  });

  it("weights a full-phrase hit above a single-term hit", () => {
    const score = createRelevanceScorer("rate limiter");
    const phraseHit = score("the rate limiter blocks bursts");
    const partialHit = score("the limiter is configurable");
    expect(phraseHit).toBeGreaterThan(partialHit);
    expect(partialHit).toBeGreaterThan(0);
  });

  it("agrees with matchesRecallQuery (shared scoring path)", () => {
    expect(matchesRecallQuery("jwt rotation", "auth")).toBe(true);
    expect(matchesRecallQuery("redis cache", "auth")).toBe(false);
  });

  it("drops stop words and sub-3-char tokens from the query", () => {
    // 'that' is a stop word; 'js' is < 3 chars. Only 'postgres' (and its alias)
    // should survive as a real term.
    const terms = recallTerms("that js postgres");
    expect(terms).not.toContain("that");
    expect(terms).not.toContain("js");
    expect(terms).toContain("postgres");
    expect(terms).toContain("postgresql"); // alias expansion

    // A query of only noise tokens matches nothing.
    const score = createRelevanceScorer("that js");
    expect(score("anything at all")).toBe(0);
  });
});
