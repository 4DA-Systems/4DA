// SPDX-License-Identifier: Apache-2.0
/**
 * Breaking changes that live in string literals: route and pattern syntax.
 *
 * Symbol matching cannot see these. axum 0.8 changed path parameters from
 * `/:single` and `/*many` to `/{single}` and `/{*many}`; a project's code
 * imports `Router`, and the old syntax sits only in strings like
 * `.route("/users/:id", ...)`. express 5 (path-to-regexp 8) and react-router
 * changed route syntax the same way.
 *
 * So: take the backticked syntax fragments on the "from" side of a breaking
 * entry ("changed from `/:single` ... to `/{single}`"), turn each into a shape
 * (identifiers become placeholders, punctuation stays literal: `/:\w+`), and
 * look for that shape inside string literals of the files that import the
 * package. Only fragments that are mostly syntax qualify; a code path like
 * `Router::route` or `res.json()` is symbol matching's job.
 */

import { promises as fs } from "node:fs";
import path from "node:path";

import type { ReportSection } from "./upgrade-impact-report.js";

/** Punctuation that makes a fragment syntax rather than a name. */
const SYNTAX_CHARS = /[/:*{}?$%@#<>]/g;
const WORD = /[A-Za-z_][\w-]*/g;
const MAX_FILES = 300;
const MAX_FILE_BYTES = 512 * 1024;
const MAX_HITS_PER_ENTRY = 5;

export interface LiteralHit {
  file: string;
  line: number;
  literal: string;
}

function escapeRegex(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Shapes of the syntax an entry says is going away. Only the text before
 * " to " when the entry reads "from X to Y", so the new syntax (which a
 * migrated project already uses) is never reported as needing a change.
 */
export function oldSyntaxShapes(entryText: string): RegExp[] {
  const fromTo = /\bfrom\b([\s\S]*?)\bto\b/i.exec(entryText);
  const scope = fromTo ? fromTo[1] : entryText;
  const shapes: RegExp[] = [];
  for (const [, fragment] of scope.matchAll(/`([^`\s]{2,40})`/g)) {
    // A path or call (`Router::route`, `res.json()`, `a.b`) is a symbol, not syntax.
    if (/^[\w$.:]+(?:\(\))?$/.test(fragment) || fragment.includes("(")) continue;
    const syntax = fragment.match(SYNTAX_CHARS) ?? [];
    const words = fragment.match(WORD) ?? [];
    if (syntax.length < 2 || words.length === 0) continue;
    let pattern = "";
    let last = 0;
    for (const m of fragment.matchAll(WORD)) {
      pattern += escapeRegex(fragment.slice(last, m.index)) + "[A-Za-z_][\\w-]*";
      last = (m.index ?? 0) + m[0].length;
    }
    pattern += escapeRegex(fragment.slice(last));
    shapes.push(new RegExp(pattern));
  }
  return shapes;
}

const STRING_LITERAL = /"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'|`(?:[^`\\]|\\.)*`/g;

/** String literals of one source file, with their line numbers. */
export function stringLiterals(source: string): Array<{ text: string; line: number }> {
  const out: Array<{ text: string; line: number }> = [];
  for (const m of source.matchAll(STRING_LITERAL)) {
    const start = m.index ?? 0;
    out.push({ text: m[0].slice(1, -1), line: source.slice(0, start).split("\n").length });
  }
  return out;
}

/**
 * Flag breaking/deprecation entries whose old syntax appears in a string
 * literal of a file that imports the package. Mutates the entries (adds
 * `touches_your_code` and `matched_literals`) and returns how many breaking
 * entries became touching, plus up to five example literals for the summary.
 */
export async function flagOldSyntax(
  sections: ReportSection[],
  files: Array<{ path: string }>,
  root: string | null,
): Promise<{ newlyTouching: number; examples: string[] }> {
  const candidates = sections
    .flatMap((s) => s.entries)
    .filter((e) => e.kind === "breaking" || e.kind === "deprecation")
    .map((entry) => ({ entry, shapes: oldSyntaxShapes(entry.text) }))
    .filter((c) => c.shapes.length > 0);
  if (!root || candidates.length === 0 || files.length === 0) return { newlyTouching: 0, examples: [] };

  const literals: Array<{ file: string; text: string; line: number }> = [];
  for (const file of files.slice(0, MAX_FILES)) {
    const abs = path.resolve(root, file.path);
    try {
      const stat = await fs.stat(abs);
      if (stat.size > MAX_FILE_BYTES) continue;
      for (const lit of stringLiterals(await fs.readFile(abs, "utf8"))) {
        // Routes and patterns have no whitespace; code and prose inside strings do (a test
        // fixture in 4DA's watcher.rs holding Rust source matched `/*many` on the first live run).
        if (lit.text.length <= 200 && !/\s|\\[nrt]/.test(lit.text)) literals.push({ file: file.path, ...lit });
      }
    } catch {
      // A file that vanished since the scan is simply not searched.
    }
  }

  let newlyTouching = 0;
  const examples = new Set<string>();
  for (const { entry, shapes } of candidates) {
    const hits: LiteralHit[] = [];
    for (const lit of literals) {
      if (hits.length >= MAX_HITS_PER_ENTRY) break;
      if (shapes.some((s) => s.test(lit.text))) hits.push({ file: lit.file, line: lit.line, literal: lit.text.slice(0, 80) });
    }
    if (hits.length === 0) continue;
    if (entry.kind === "breaking" && !entry.touches_your_code) newlyTouching++;
    entry.touches_your_code = true;
    entry.matched_literals = hits;
    hits.forEach((h) => examples.size < 5 && examples.add(`"${h.literal}"`));
  }
  return { newlyTouching, examples: [...examples] };
}
