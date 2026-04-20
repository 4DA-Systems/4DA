// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Version Resolver
 *
 * Extracts exact dependency versions from lock files.
 * Priority: lock file (exact) > manifest (range/specifier).
 * Uses only Node.js built-ins — no external parsers.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import type { OsvEcosystem, ResolvedDependency } from "./types.js";

const ECOSYSTEM_MAP: Record<string, OsvEcosystem> = {
  npm: "npm",
  rust: "crates.io",
  python: "PyPI",
  go: "Go",
  java: "Maven",
  dotnet: "NuGet",
  ruby: "RubyGems",
  php: "Packagist",
};

export function mapEcosystem(language: string): OsvEcosystem {
  return ECOSYSTEM_MAP[language] || "npm";
}

export function resolveVersions(
  cwd: string,
  deps: string[],
  devDeps: string[],
  language: string,
): ResolvedDependency[] {
  const ecosystem = mapEcosystem(language);
  const results: ResolvedDependency[] = [];

  const resolvers: Record<string, () => Map<string, string>> = {
    npm: () => resolveNpm(cwd),
    "crates.io": () => resolveRust(cwd),
    PyPI: () => resolvePython(cwd),
    Go: () => resolveGo(cwd),
  };

  const resolver = resolvers[ecosystem];
  const versionMap = resolver ? resolver() : new Map<string, string>();

  for (const name of deps) {
    results.push({
      name,
      version: versionMap.get(name) || null,
      ecosystem,
      isDev: false,
    });
  }

  for (const name of devDeps) {
    results.push({
      name,
      version: versionMap.get(name) || null,
      ecosystem,
      isDev: true,
    });
  }

  return results;
}

// =============================================================================
// npm: package-lock.json > pnpm-lock.yaml > yarn.lock > package.json ranges
// =============================================================================

function resolveNpm(cwd: string): Map<string, string> {
  const versions = new Map<string, string>();

  // Try package-lock.json first (npm)
  const lockPath = path.join(cwd, "package-lock.json");
  if (fs.existsSync(lockPath)) {
    try {
      const lock = JSON.parse(fs.readFileSync(lockPath, "utf-8"));
      // v2/v3 format: packages["node_modules/name"].version
      if (lock.packages) {
        for (const [key, value] of Object.entries(lock.packages)) {
          const name = key.replace(/^node_modules\//, "");
          if (name && (value as { version?: string }).version) {
            versions.set(name, (value as { version: string }).version);
          }
        }
      }
      // v1 format fallback: dependencies.name.version
      if (versions.size === 0 && lock.dependencies) {
        for (const [name, value] of Object.entries(lock.dependencies)) {
          if ((value as { version?: string }).version) {
            versions.set(name, (value as { version: string }).version);
          }
        }
      }
      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // Try pnpm-lock.yaml (line-by-line, no YAML parser)
  const pnpmLockPath = path.join(cwd, "pnpm-lock.yaml");
  if (fs.existsSync(pnpmLockPath)) {
    try {
      const content = fs.readFileSync(pnpmLockPath, "utf-8");

      // pnpm v9 importers format: dependency name on one line, version on the next
      // Example:
      //   '@modelcontextprotocol/sdk':
      //     specifier: ^1.10.0
      //     version: 1.26.0(zod@4.3.6)
      const lines = content.split("\n");
      let currentPkg: string | null = null;

      for (const line of lines) {
        // Match dependency name (with or without quotes, scoped packages)
        const pkgMatch = line.match(/^\s{4,8}'?(@?[^':]+)'?:\s*$/);
        if (pkgMatch) {
          currentPkg = pkgMatch[1].trim();
          continue;
        }
        // Match version line under a dependency
        if (currentPkg) {
          const versionMatch = line.match(/^\s+version:\s+['"]?(\d+\.\d+[^('"\s]*)['"]?/);
          if (versionMatch) {
            versions.set(currentPkg, versionMatch[1]);
            currentPkg = null;
            continue;
          }
          // specifier line — skip, wait for version
          if (line.match(/^\s+specifier:/)) continue;
          // Any other line resets current package context
          if (!line.match(/^\s/)) currentPkg = null;
        }
      }

      // Also try packages section for pnpm v6-v8 format: '/name@version:'
      const pkgRegex = /^\s*['/]?([^@\s][^@]*)@(\d+[^:('"]*)/gm;
      let match;
      while ((match = pkgRegex.exec(content)) !== null) {
        const name = match[1].replace(/^\//, "").trim();
        const version = match[2].trim().replace(/['":]/g, "");
        if (name && version && /^\d/.test(version) && !versions.has(name)) {
          versions.set(name, version);
        }
      }

      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // Try yarn.lock
  const yarnLockPath = path.join(cwd, "yarn.lock");
  if (fs.existsSync(yarnLockPath)) {
    try {
      const content = fs.readFileSync(yarnLockPath, "utf-8");
      // Format: "name@range":\n  version "x.y.z"
      const blocks = content.split(/\n(?=\S)/);
      for (const block of blocks) {
        const headerMatch = block.match(/^"?([^@\s"]+)@/);
        const versionMatch = block.match(/^\s+version\s+"([^"]+)"/m);
        if (headerMatch && versionMatch) {
          versions.set(headerMatch[1], versionMatch[1]);
        }
      }
      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // Fallback: extract version ranges from package.json
  const pkgPath = path.join(cwd, "package.json");
  if (fs.existsSync(pkgPath)) {
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf-8"));
      const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
      for (const [name, spec] of Object.entries(allDeps)) {
        const version = extractVersionFromSpec(spec as string);
        if (version) versions.set(name, version);
      }
    } catch { /* skip */ }
  }

  return versions;
}

// =============================================================================
// Rust: Cargo.lock
// =============================================================================

function resolveRust(cwd: string): Map<string, string> {
  const versions = new Map<string, string>();

  const lockPath = path.join(cwd, "Cargo.lock");
  if (!fs.existsSync(lockPath)) {
    // Fallback: parse Cargo.toml version strings
    return resolveRustFromManifest(cwd);
  }

  try {
    const content = fs.readFileSync(lockPath, "utf-8");
    // [[package]]\nname = "serde"\nversion = "1.0.193"
    const packageRegex = /\[\[package\]\]\s*\nname\s*=\s*"([^"]+)"\s*\nversion\s*=\s*"([^"]+)"/g;
    let match;
    while ((match = packageRegex.exec(content)) !== null) {
      versions.set(match[1], match[2]);
    }
  } catch { /* skip */ }

  return versions;
}

function resolveRustFromManifest(cwd: string): Map<string, string> {
  const versions = new Map<string, string>();
  const cargoPath = path.join(cwd, "Cargo.toml");
  if (!fs.existsSync(cargoPath)) return versions;

  try {
    const content = fs.readFileSync(cargoPath, "utf-8");
    // name = "1.0" or name = { version = "1.0", ... }
    const depRegex = /^([a-zA-Z_][\w-]*)\s*=\s*(?:"([^"]+)"|.*?version\s*=\s*"([^"]+)")/gm;
    let match;
    while ((match = depRegex.exec(content)) !== null) {
      const name = match[1];
      const version = match[2] || match[3];
      if (version && /^\d/.test(version)) {
        versions.set(name, version);
      }
    }
  } catch { /* skip */ }

  return versions;
}

// =============================================================================
// Python: poetry.lock > Pipfile.lock > requirements.txt
// =============================================================================

function resolvePython(cwd: string): Map<string, string> {
  const versions = new Map<string, string>();

  // poetry.lock
  const poetryLock = path.join(cwd, "poetry.lock");
  if (fs.existsSync(poetryLock)) {
    try {
      const content = fs.readFileSync(poetryLock, "utf-8");
      const packageRegex = /\[\[package\]\]\s*\nname\s*=\s*"([^"]+)"\s*\nversion\s*=\s*"([^"]+)"/g;
      let match;
      while ((match = packageRegex.exec(content)) !== null) {
        versions.set(match[1].toLowerCase(), match[2]);
      }
      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // Pipfile.lock
  const pipfileLock = path.join(cwd, "Pipfile.lock");
  if (fs.existsSync(pipfileLock)) {
    try {
      const lock = JSON.parse(fs.readFileSync(pipfileLock, "utf-8"));
      const sections = [lock.default, lock.develop].filter(Boolean);
      for (const section of sections) {
        for (const [name, info] of Object.entries(section)) {
          const version = (info as { version?: string }).version;
          if (version) {
            versions.set(name.toLowerCase(), version.replace(/^==/, ""));
          }
        }
      }
      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // requirements.txt pinned versions
  const reqPath = path.join(cwd, "requirements.txt");
  if (fs.existsSync(reqPath)) {
    try {
      const content = fs.readFileSync(reqPath, "utf-8");
      for (const line of content.split("\n")) {
        const match = line.trim().match(/^([a-zA-Z0-9_-]+)\s*==\s*(.+)/);
        if (match) {
          versions.set(match[1].toLowerCase(), match[2].trim());
        }
      }
    } catch { /* skip */ }
  }

  return versions;
}

// =============================================================================
// Go: go.sum > go.mod
// =============================================================================

function resolveGo(cwd: string): Map<string, string> {
  const versions = new Map<string, string>();

  // go.sum has exact versions
  const goSumPath = path.join(cwd, "go.sum");
  if (fs.existsSync(goSumPath)) {
    try {
      const content = fs.readFileSync(goSumPath, "utf-8");
      for (const line of content.split("\n")) {
        const match = line.match(/^(\S+)\s+(v[\d.]+)/);
        if (match) {
          versions.set(match[1], match[2]);
        }
      }
      if (versions.size > 0) return versions;
    } catch { /* fall through */ }
  }

  // Fallback: go.mod require block
  const goModPath = path.join(cwd, "go.mod");
  if (fs.existsSync(goModPath)) {
    try {
      const content = fs.readFileSync(goModPath, "utf-8");
      const requireMatch = content.match(/require\s*\(([\s\S]*?)\)/);
      if (requireMatch) {
        for (const line of requireMatch[1].split("\n")) {
          const match = line.trim().match(/^(\S+)\s+(v[\d.]+)/);
          if (match) {
            versions.set(match[1], match[2]);
          }
        }
      }
    } catch { /* skip */ }
  }

  return versions;
}

// =============================================================================
// Helpers
// =============================================================================

/**
 * Extract a usable version from an npm version specifier.
 * "^4.18.2" -> "4.18.2", "~1.0.0" -> "1.0.0", ">=2.0.0" -> "2.0.0"
 */
function extractVersionFromSpec(spec: string): string | null {
  if (!spec) return null;
  const match = spec.match(/(\d+\.\d+\.\d+(?:-[\w.]+)?)/);
  return match ? match[1] : null;
}
