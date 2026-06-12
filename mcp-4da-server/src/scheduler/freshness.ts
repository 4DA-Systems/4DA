/**
 * Freshness watermarks (S2) — when was each named feed last refreshed, and is it stale?
 *
 * A tiny persistent map: mark a name fresh at a moment, ask how stale it is against a budget.
 * Persistence is an explicit save()/load() to a JSON file so the scheduler can survive a
 * restart without inventing freshness it does not have — a missing file simply means
 * "nothing is known", which reads as stale (the honest default).
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

export interface Freshness {
  markFresh(name: string, at?: number): void;
  lastFresh(name: string): number | null;
  isStale(name: string, maxAgeMs: number, now?: number): boolean;
  save(): void;
  load(): void;
}

export function createFreshness(filePath?: string): Freshness {
  let marks = new Map<string, number>();

  return {
    markFresh(name: string, at?: number): void {
      marks.set(name, at ?? Date.now());
    },

    lastFresh(name: string): number | null {
      return marks.get(name) ?? null;
    },

    isStale(name: string, maxAgeMs: number, now?: number): boolean {
      const last = marks.get(name);
      if (last === undefined) {
        return true; // never refreshed is the stalest state there is
      }
      return (now ?? Date.now()) - last > maxAgeMs;
    },

    save(): void {
      if (!filePath) return;
      mkdirSync(dirname(filePath), { recursive: true });
      writeFileSync(filePath, JSON.stringify(Object.fromEntries(marks)), "utf8");
    },

    load(): void {
      if (!filePath || !existsSync(filePath)) return;
      const raw = JSON.parse(readFileSync(filePath, "utf8")) as Record<string, number>;
      marks = new Map(Object.entries(raw));
    },
  };
}
