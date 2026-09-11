// SPDX-License-Identifier: Apache-2.0
/**
 * pnpm and yarn lockfile readers: exact versions, no YAML parser.
 *
 * Measured 2026-09-11: the previous pnpm reader matched package keys with
 * `/^\s{2}'?(.+?)@(\d…)/gm` over the whole file. `\s` also matches a newline,
 * and pnpm writes a blank line between entries, so each match began on the
 * blank line and kept the next key's opening quote. Every scoped package
 * outside the importer blocks reached OSV as `'@scope/name` and matched
 * nothing: 525 names across five lockfiles in this repository, including
 * `@humanfs/node@0.16.7` (GHSA-p498-v437-472g), which GitHub and the app both
 * flagged. The test fixture had no blank lines, so it passed. The yarn reader's
 * header pattern could not start with `@`, so it dropped every scoped package,
 * and it never read yarn berry's `version: x` lines.
 *
 * So indentation is read by column on whole lines, never with `\s` across a
 * line break, and a key that does not pin a registry version (git, tarball,
 * file, link) is dropped instead of being queried as a package name.
 */

const DEPENDENCY_BLOCKS = new Set(["dependencies:", "devDependencies:", "optionalDependencies:"]);

/** A bare npm package name, scoped or not: no whitespace, no `/` beyond the scope, no version. */
const PACKAGE_NAME = /^(?:@[^\s/@]+\/)?[^\s/@]+$/;

function unquote(s: string): string {
  return s.length >= 2 && (s[0] === "'" || s[0] === '"') && s[s.length - 1] === s[0] ? s.slice(1, -1) : s;
}

/** An exact version with pnpm's peer suffixes removed: v6/v9 `1.2.3(react@18)`, v5 `1.2.3_react@18`. */
function exactVersion(raw: string): string | null {
  const version = unquote(raw.trim()).split("(")[0].split("_")[0].trim();
  return /^\d/.test(version) ? version : null;
}

/**
 * Splits one `packages:` or `snapshots:` key into [name, version]:
 * v9 `'@scope/pkg@1.2.3(peer@4.5.6)'`, v6 `/pkg@1.2.3`, v5 `/@scope/pkg/1.2.3_peer@4.5.6`.
 * Returns null for a key that does not pin a registry version.
 */
export function parsePnpmKey(rawKey: string): [string, string] | null {
  let key = unquote(rawKey.trim());
  if (key.startsWith("/")) key = key.slice(1);
  const peers = key.indexOf("(");
  if (peers >= 0) key = key.slice(0, peers);

  // v6 and v9: the separator is the first `@` after a scoped name's own leading `@`.
  const at = key.indexOf("@", key.startsWith("@") ? 1 : 0);
  if (at > 0) {
    const name = key.slice(0, at);
    const version = exactVersion(key.slice(at + 1));
    if (PACKAGE_NAME.test(name) && version) return [name, version];
  }

  // v5: `name/version`, with any peers after an underscore.
  const v5 = /^((?:@[^/@\s]+\/)?[^/@\s]+)\/(\d[^/]*)$/.exec(key);
  if (v5) {
    const version = exactVersion(v5[2]);
    if (version) return [v5[1], version];
  }
  return null;
}

/**
 * Reads a pnpm lockfile (v5.x, v6 or v9) into `versions`; the first version seen wins.
 *
 * Direct pins are read first, from the dependency blocks: `importers:` → `  <dir>:` →
 * `    dependencies:` in a workspace, or a top-level `dependencies:` block in a v5/v6
 * single-project file. A direct pin therefore wins over a transitive copy of the same
 * name. Every other package comes from a key exactly two columns deep under
 * `packages:` or `snapshots:`.
 */
export function readPnpmLock(content: string, versions: Map<string, string>): void {
  const transitive: Array<[string, string]> = [];
  const setFirst = (name: string, version: string): void => {
    if (!versions.has(name)) versions.set(name, version);
  };
  let section = "";
  let blockIndent = -1; // column of the dependency block being read, or -1
  let pending: string | null = null; // v6/v9: a name whose `version:` line follows

  for (const line of content.split(/\r?\n/)) {
    const text = line.trim();
    if (text === "" || text.startsWith("#")) continue;
    const indent = line.length - line.trimStart().length;

    if (indent === 0) {
      section = text.endsWith(":") ? text.slice(0, -1) : "";
      blockIndent = DEPENDENCY_BLOCKS.has(text) ? 0 : -1;
      pending = null;
      continue;
    }
    if (section === "packages" || section === "snapshots") {
      if (indent === 2 && text.endsWith(":")) {
        const parsed = parsePnpmKey(text.slice(0, -1));
        if (parsed) transitive.push(parsed);
      }
      continue;
    }
    if (section === "importers" && indent <= 4) {
      blockIndent = indent === 4 && DEPENDENCY_BLOCKS.has(text) ? 4 : -1;
      pending = null;
      continue;
    }
    if (blockIndent < 0) continue;

    if (indent === blockIndent + 2) {
      pending = null;
      const entry = /^(?:'([^']+)'|"([^"]+)"|([^\s:'"]+)):(?:\s+(.*))?$/.exec(text);
      if (!entry) continue;
      const name = entry[1] ?? entry[2] ?? entry[3];
      const value = entry[4];
      if (value === undefined || value === "") {
        pending = name;
      } else {
        const version = exactVersion(value); // v5: `name: 1.2.3_peer@4.5.6`
        if (version) setFirst(name, version);
      }
    } else if (indent === blockIndent + 4 && pending && text.startsWith("version:")) {
      const version = exactVersion(text.slice("version:".length));
      if (version) setFirst(pending, version);
      pending = null;
    }
  }

  for (const [name, version] of transitive) setFirst(name, version);
}

/**
 * Reads a yarn lockfile (v1 or berry) into `versions`.
 * v1: `"@scope/pkg@^1.0.0", "@scope/pkg@^1.1.0":` then `  version "1.2.3"`.
 * berry: `"@scope/pkg@npm:^1.0.0":` then `  version: 1.2.3`.
 * Workspace, link, portal, file, exec and patch entries do not name a registry version.
 */
export function readYarnLock(content: string, versions: Map<string, string>): void {
  for (const block of content.split(/\r?\n(?=\S)/)) {
    const firstLine = block.split(/\r?\n/)[0];
    const header = /^"?(@?[^@\s"]+)@/.exec(firstLine);
    if (!header || header[1].startsWith("__")) continue;
    if (/@(?:workspace|link|portal|file|exec|patch):/.test(firstLine)) continue;
    const line = /^\s+version:?\s+"?([^"\s]+)"?\s*$/m.exec(block);
    const version = line ? exactVersion(line[1]) : null;
    if (version) versions.set(header[1], version);
  }
}
