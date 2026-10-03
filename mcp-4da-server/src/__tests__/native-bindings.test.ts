// SPDX-License-Identifier: Apache-2.0
/**
 * The native-module check opens a database instead of trusting the import.
 * 2026-10-03, node:24 + npm 12.2.0: `npm install` blocked better-sqlite3's
 * install script, `--doctor` said "loaded successfully", and every tool
 * failed with "Could not locate the bindings file".
 */
import { describe, it, expect } from "vitest";
import Database from "better-sqlite3";
import { checkNativeBindings, isNativeBindingError, nativeBindingMessage, NATIVE_BINDING_FIX } from "../native-bindings.js";

const MISSING = new Error(
  "Could not locate the bindings file. Tried:\n → /t/node_modules/better-sqlite3/build/better_sqlite3.node\n → /t/node_modules/better-sqlite3/build/Release/better_sqlite3.node",
);

describe("native bindings", () => {
  it("passes when a database really opens", () => {
    expect(checkNativeBindings(Database)).toBeNull();
  });

  it("fails with the fix when opening throws (import alone is not proof)", () => {
    const Broken = function () { throw MISSING; } as unknown as typeof Database;
    const problem = checkNativeBindings(Broken);
    expect(problem).toContain("Could not locate the bindings file");
    expect(problem).toContain("npm install-scripts approve better-sqlite3");
    expect(problem).toContain("npm config set allow-scripts=better-sqlite3 --location=user");
    expect(problem).toContain("npx clear-npx-cache");
    expect(problem).not.toContain("Tried:");
  });

  it("recognises missing or mismatched modules, and nothing else", () => {
    expect(isNativeBindingError(MISSING)).toBe(true);
    expect(isNativeBindingError(new Error("The module was compiled against a different Node.js version using NODE_MODULE_VERSION 127"))).toBe(true);
    expect(isNativeBindingError(new Error("SQLITE_CANTOPEN: unable to open database file"))).toBe(false);
    expect(nativeBindingMessage(MISSING)).toBe(`better-sqlite3's native module is not usable (Could not locate the bindings file.). ${NATIVE_BINDING_FIX}`);
  });
});
