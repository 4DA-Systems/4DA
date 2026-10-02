// SPDX-License-Identifier: Apache-2.0
/**
 * The MCP server must look for the desktop app's database exactly where the
 * deployed app writes it (src-tauri/src/state.rs). On Linux it looked in
 * ~/.local/share/com.4da.app/data, a directory the app never writes (the app
 * uses $XDG_DATA_HOME/4da/data), so desktop mode never activated there.
 */
import { describe, expect, it } from "vitest";

import { desktopAppDbPaths } from "../db.js";

describe("desktopAppDbPaths mirrors the app's get_db_path", () => {
  it("Linux: $XDG_DATA_HOME/4da/data first, the old com.4da.app path last", () => {
    expect(desktopAppDbPaths("linux", { XDG_DATA_HOME: "/xdg" }, "/home/u")).toEqual([
      "/xdg/4da/data/4da.db",
      "/home/u/.local/share/com.4da.app/data/4da.db",
    ]);
    expect(desktopAppDbPaths("linux", {}, "/home/u")[0]).toBe("/home/u/.local/share/4da/data/4da.db");
  });

  it("Windows and macOS: the com.4da.app data directory", () => {
    expect(desktopAppDbPaths("win32", { APPDATA: "C:\\Users\\u\\AppData\\Roaming" }, "C:\\Users\\u")).toEqual([
      "C:\\Users\\u\\AppData\\Roaming\\com.4da.app\\data\\4da.db",
    ]);
    expect(desktopAppDbPaths("darwin", {}, "/Users/u")).toEqual(["/Users/u/Library/Application Support/com.4da.app/data/4da.db"]);
  });

  it("FOURDA_DATA_DIR, the app's own data-dir override, wins", () => {
    expect(desktopAppDbPaths("linux", { FOURDA_DATA_DIR: "/srv/4da" }, "/home/u")[0]).toBe("/srv/4da/4da.db");
  });
});
