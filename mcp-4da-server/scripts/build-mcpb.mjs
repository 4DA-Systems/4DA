// SPDX-License-Identifier: Apache-2.0
/**
 * Build a Claude Desktop extension (.mcpb) for @4da/mcp-server.
 *
 * The bundle is PLATFORM-SPECIFIC: better-sqlite3 ships a native prebuild for
 * the platform that ran `npm install`, so each OS/arch needs its own .mcpb
 * (built on that OS in CI). The manifest declares the platform accordingly.
 *
 * Usage:  node scripts/build-mcpb.mjs        (from mcp-4da-server/, after `pnpm build`)
 * Output: dist-mcpb/4da-mcp-server-<version>-<platform>-<arch>.mcpb
 */
import { execSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const platform = process.platform; // win32 | darwin | linux
const arch = process.arch;

if (!existsSync(join(root, "dist", "index.js"))) {
  console.error("dist/index.js missing — run `pnpm build` first");
  process.exit(1);
}

const outDir = join(root, "dist-mcpb");
const stage = join(outDir, "stage");
rmSync(outDir, { recursive: true, force: true });
mkdirSync(join(stage, "server"), { recursive: true });

// 1. Built server code. SERVER_VERSION resolves join(__dirname, "..", "package.json")
//    → server/package.json below, which doubles as the npm-install manifest.
cpSync(join(root, "dist"), join(stage, "server", "dist"), { recursive: true });
writeFileSync(
  join(stage, "server", "package.json"),
  JSON.stringify(
    { name: pkg.name, version: pkg.version, type: "module", dependencies: pkg.dependencies },
    null,
    2,
  ),
);

// 2. Production node_modules via a clean npm install (npm, not pnpm: pnpm's
//    symlinked layout does not survive zip packing). This is what pins the
//    bundle to this OS/arch — better-sqlite3's prebuild lands here.
execSync("npm install --omit=dev --no-audit --no-fund --ignore-scripts=false", {
  cwd: join(stage, "server"),
  stdio: "inherit",
});

// 3. Manifest — generated from package.json so versions can never drift.
const standaloneTools = [
  { name: "vulnerability_scan", description: "Scan the project's dependencies for known CVEs via OSV.dev — severity, fix versions, upgrade commands." },
  { name: "dependency_health", description: "Version freshness, deprecations, and known issues across npm, Rust, Python, Go." },
  { name: "upgrade_planner", description: "Ranked upgrade plan: quick wins vs breaking majors." },
  { name: "what_should_i_know", description: "Pre-task briefing: advisories and decisions relevant to the task at hand." },
  { name: "ecosystem_pulse", description: "What moved in the project's ecosystem lately." },
  { name: "get_context", description: "The detected stack, so the agent stops guessing versions." },
  { name: "decision_memory", description: "Record and recall architecture decisions across sessions." },
  { name: "check_decision_alignment", description: "Check a proposed change against recorded decisions." },
  { name: "agent_memory", description: "Cross-session persistent memory for agent work." },
];

const manifest = {
  manifest_version: "0.2",
  name: "4da-mcp-server",
  display_name: "4DA — Developer Intelligence",
  version: pkg.version,
  description: "Stack-aware developer intelligence: CVE scans, dependency health, upgrade plans, decision memory. Privacy-first — only public package names leave your machine.",
  long_description:
    "Point 4DA at a project folder and your agent gets nine tools: vulnerability scanning (full lockfile tree against OSV.dev), dependency health, ranked upgrade plans, pre-task briefings, ecosystem pulse, detected stack context, decision memory and alignment checks, and cross-session agent memory. The only data that ever leaves your machine is public package names and versions sent to OSV.dev and public registries — your code, paths, and prompts never do.",
  author: { name: "4DA Systems", url: "https://4da.ai" },
  homepage: "https://4da.ai/mcp/",
  documentation: "https://4da.ai/mcp/",
  support: "https://github.com/4DA-Systems/4DA/issues",
  license: "Apache-2.0",
  keywords: ["security", "dependencies", "cve", "developer-intelligence", "privacy"],
  repository: { type: "git", url: "https://github.com/4DA-Systems/4DA" },
  server: {
    type: "node",
    entry_point: "server/dist/index.js",
    mcp_config: {
      command: "node",
      args: ["${__dirname}/server/dist/index.js"],
      env: {
        FOURDA_PROJECT_DIR: "${user_config.project_dir}",
      },
    },
  },
  tools: standaloneTools,
  user_config: {
    project_dir: {
      type: "directory",
      title: "Project folder",
      description: "The code folder 4DA scans (manifests + lockfiles). Only public package names and versions are ever sent anywhere.",
      required: true,
    },
  },
  compatibility: {
    platforms: [platform],
    runtimes: { node: ">=18.0.0" },
  },
};
writeFileSync(join(stage, "manifest.json"), JSON.stringify(manifest, null, 2));

// 4. Pack with the official CLI (validates the manifest as part of packing).
const outFile = join(outDir, `4da-mcp-server-${pkg.version}-${platform}-${arch}.mcpb`);
execSync(`npx -y @anthropic-ai/mcpb pack "${stage}" "${outFile}"`, { stdio: "inherit" });
console.log(`\nBuilt ${outFile}`);
