// SPDX-License-Identifier: Apache-2.0
/**
 * Tool-argument validation against each tool's published inputSchema, and the
 * text hygiene applied to every tool result.
 *
 * Validation: a call with a wrong type, an out-of-enum value or an unknown
 * parameter name used to reach the executor anyway. Measured 2026-10-02:
 * `project_path: 42` surfaced as a raw Node TypeError, an invalid
 * `severity_filter` was silently ignored, and a misspelled parameter vanished.
 * The spec (2025-11-25, SEP-1303) asks for input errors as tool execution
 * errors the model can read and correct; the message names the fix.
 *
 * Hygiene: results carry third-party text — advisory summaries, changelog
 * entries, headlines, package deprecation messages. Characters that render as
 * nothing (zero-width, bidi overrides, Unicode tag characters) and control
 * characters are the raw material of hidden prompt-injection payloads and have
 * no legitimate use in this data, so they are removed from every string.
 */

interface PropertySchema {
  type?: string;
  enum?: readonly unknown[];
  items?: { type?: string };
  minimum?: number;
  maximum?: number;
}

interface InputSchema {
  properties?: Record<string, unknown>;
  required?: readonly string[];
}

function typeOf(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function matchesType(value: unknown, type: string | undefined): boolean {
  if (!type) return true;
  if (type === "integer") return typeof value === "number" && Number.isInteger(value);
  if (type === "number") return typeof value === "number" && Number.isFinite(value);
  return typeOf(value) === type;
}

/** An actionable message for the first problem with `args`, or null when they are valid. */
export function validateToolArgs(toolName: string, schema: InputSchema, args: Record<string, unknown>): string | null {
  const properties = (schema.properties ?? {}) as Record<string, PropertySchema>;
  const known = Object.keys(properties);

  for (const name of schema.required ?? []) {
    if (args[name] === undefined || args[name] === null || args[name] === "") {
      return `${toolName}: missing required parameter "${name}". Required: ${(schema.required ?? []).join(", ")}.`;
    }
  }

  for (const [name, value] of Object.entries(args)) {
    const prop = properties[name];
    if (!prop) {
      return `${toolName}: unknown parameter "${name}". Valid parameters: ${known.join(", ") || "(none)"}.`;
    }
    if (value === undefined || value === null) continue;
    if (!matchesType(value, prop.type)) {
      return `${toolName}: "${name}" must be ${prop.type === "array" ? "an array" : `a ${prop.type}`}, got ${typeOf(value)} ${JSON.stringify(value)}.`;
    }
    if (prop.enum && !prop.enum.includes(value)) {
      return `${toolName}: "${name}" must be one of ${prop.enum.map((e) => JSON.stringify(e)).join(", ")}; got ${JSON.stringify(value)}.`;
    }
    if (typeof value === "number") {
      if (prop.minimum !== undefined && value < prop.minimum) return `${toolName}: "${name}" must be >= ${prop.minimum}; got ${value}.`;
      if (prop.maximum !== undefined && value > prop.maximum) return `${toolName}: "${name}" must be <= ${prop.maximum}; got ${value}.`;
    }
    if (Array.isArray(value) && prop.items?.type) {
      const bad = value.findIndex((item) => !matchesType(item, prop.items?.type));
      if (bad >= 0) {
        return `${toolName}: every "${name}" entry must be a ${prop.items.type}; entry ${bad} is ${typeOf(value[bad])}.`;
      }
    }
  }
  return null;
}

// Control characters except tab/newline/carriage return; zero-width and joiner
// characters; bidi embeddings, overrides and isolates; word joiner and
// invisible operators; the BOM; Unicode tag characters (U+E0000..U+E007F).
const INVISIBLE =
  /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F​-‏‪-‮⁠-⁤⁦-⁩﻿]|\uDB40[\uDC00-\uDC7F]/g;

/** Remove invisible and control characters from one string. */
export function cleanText(text: string): string {
  return text.replace(INVISIBLE, "");
}

/** Deep copy of a JSON-like value with every string cleaned. */
export function cleanStrings<T>(value: T): T {
  if (typeof value === "string") return cleanText(value) as T;
  if (Array.isArray(value)) return value.map((v) => cleanStrings(v)) as T;
  if (value && typeof value === "object" && (value as object).constructor === Object) {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) out[k] = cleanStrings(v);
    return out as T;
  }
  return value;
}
