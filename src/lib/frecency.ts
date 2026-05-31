// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Frecency — frequency + recency ranking for command-search picks.
 *
 * Persisted to localStorage so the palette "gets sharper every day": the more
 * (and more recently) you choose a result, the higher it ranks. This is the
 * product thesis applied to the app's own navigation — typing "b" floats the
 * view you actually open, because you've opened it before.
 *
 * The boost is bounded and added to a result's base match score, so it nudges
 * ordering within a group without ever overriding a strong textual match.
 */

const STORE_KEY = 'cmdk:frecency:v1';
const MAX_ENTRIES = 200;
const HALF_LIFE_MS = 14 * 24 * 60 * 60 * 1000; // recency half-life: 14 days
const MAX_BOOST = 0.49; // stays below 0.5 so it can't leapfrog a full-point match

interface PickRecord {
  count: number;
  lastUsed: number;
}
type FrecencyMap = Record<string, PickRecord>;

let cache: FrecencyMap | null = null;

function load(): FrecencyMap {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    return raw ? (JSON.parse(raw) as FrecencyMap) : {};
  } catch {
    return {};
  }
}

function persist(map: FrecencyMap): void {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(map));
  } catch {
    /* private mode / quota / no storage — frecency is best-effort */
  }
}

function data(): FrecencyMap {
  if (cache === null) cache = load();
  return cache;
}

/** Record that the user selected the result with this id. */
export function recordPick(id: string, now: number = Date.now()): void {
  const map = data();
  const rec = map[id] ?? { count: 0, lastUsed: 0 };
  rec.count += 1;
  rec.lastUsed = now;
  map[id] = rec;

  // Keep only the most-recently-used entries so storage stays bounded.
  const ids = Object.keys(map);
  if (ids.length > MAX_ENTRIES) {
    ids.sort((a, b) => map[b]!.lastUsed - map[a]!.lastUsed);
    for (const stale of ids.slice(MAX_ENTRIES)) delete map[stale];
  }
  persist(map);
}

/**
 * Ranking boost in [0, MAX_BOOST]: grows with pick count (log) and decays with
 * recency (exponential half-life). Zero for never-picked ids.
 */
export function frecencyBoost(id: string, now: number = Date.now()): number {
  const rec = data()[id];
  if (!rec) return 0;
  const recency = Math.pow(0.5, (now - rec.lastUsed) / HALF_LIFE_MS); // 1 → 0
  const frequency = Math.log2(rec.count + 1); // 1, 1.58, 2, …
  return Math.min(MAX_BOOST, 0.12 * frequency * recency);
}

/** Test-only: drop the in-memory cache so a test can re-read localStorage. */
export function __resetFrecencyForTests(): void {
  cache = null;
}
