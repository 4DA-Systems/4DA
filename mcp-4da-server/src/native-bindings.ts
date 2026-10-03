// SPDX-License-Identifier: Apache-2.0
/**
 * better-sqlite3's native module: a real check and an actionable fix.
 *
 * `import("better-sqlite3")` succeeds even when the compiled module is
 * missing; the failure comes at the first `new Database()`. Measured
 * 2026-10-03 on node:24-bookworm-slim with npm 12.2.0, which blocks
 * dependency install scripts not covered by `allowScripts`: `npm install`
 * (local or -g) left no better_sqlite3.node, `--doctor` still reported the
 * bindings as loaded, and every tool failed with "Could not locate the
 * bindings file" and no hint why. `npx @4da/mcp-server` was unaffected.
 */

import type BetterSqlite3 from "better-sqlite3";

// Each remedy measured on npm 12.2.0 (node:24-bookworm-slim), 2026-10-03.
export const NATIVE_BINDING_FIX =
  "npm 12+ blocks dependency install scripts unless allowed. With npx: allow it once with " +
  "`npm config set allow-scripts=better-sqlite3 --location=user` (or run `npx --allow-scripts=better-sqlite3 @4da/mcp-server`), " +
  "then clear the npx cache (`npx clear-npx-cache`, or delete the _npx folder in your npm cache). " +
  "In an npm install: `npm install-scripts approve better-sqlite3`, then `npm rebuild better-sqlite3`. " +
  "Without a prebuilt binary for your platform the rebuild needs a C++ toolchain " +
  "(macOS: xcode-select --install; Debian/Ubuntu: apt install build-essential python3; Windows: Visual Studio Build Tools).";

/** Errors that mean the compiled module is absent or built for another Node/platform. */
export function isNativeBindingError(err: unknown): boolean {
  const msg = err instanceof Error ? err.message : String(err);
  return /bindings file|better_sqlite3\.node|NODE_MODULE_VERSION|compiled against a different Node\.js version|invalid ELF header|not a valid Win32 application/i.test(msg);
}

/** The first line of a native-binding error, with the fix appended. */
export function nativeBindingMessage(err: unknown): string {
  const first = (err instanceof Error ? err.message : String(err)).split("\n")[0].replace(/\s*Tried:\s*$/, "").trim();
  return `better-sqlite3's native module is not usable (${first}). ${NATIVE_BINDING_FIX}`;
}

/** Null when an in-memory database opens; otherwise why not, with the fix. */
export function checkNativeBindings(Database: typeof BetterSqlite3): string | null {
  try {
    new Database(":memory:").close();
    return null;
  } catch (err) {
    return nativeBindingMessage(err);
  }
}
