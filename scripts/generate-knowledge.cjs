#!/usr/bin/env node
/**
 * generate-knowledge.cjs
 *
 * Scans the 4DA codebase and generates always-current knowledge manifests
 * that expert agents read at startup instead of relying on baked-in knowledge.
 *
 * Output: .claude/knowledge/*.md (7 manifests)
 *
 * Usage:  node scripts/generate-knowledge.cjs
 * Auto:   Runs at session start via hook
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const KNOWLEDGE_DIR = path.join(ROOT, ".claude", "knowledge");
const RUST_SRC = path.join(ROOT, "src-tauri", "src");
const REACT_SRC = path.join(ROOT, "src");
const LIB_RS = path.join(RUST_SRC, "lib.rs");
const COMMANDS_TS = path.join(REACT_SRC, "lib", "commands.ts");

// Ensure output dir
if (!fs.existsSync(KNOWLEDGE_DIR)) {
  fs.mkdirSync(KNOWLEDGE_DIR, { recursive: true });
}

const TIMESTAMP = new Date().toISOString().replace("T", " ").slice(0, 19);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function walkDir(dir, extensions) {
  const results = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (["target", "node_modules", ".git", "dist"].includes(entry.name))
        continue;
      results.push(...walkDir(full, extensions));
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name);
      if (extensions.includes(ext)) {
        results.push(full);
      }
    }
  }
  return results;
}

function lineCount(filepath) {
  try {
    return fs.readFileSync(filepath, "utf-8").split("\n").length;
  } catch {
    return 0;
  }
}

function relPath(filepath) {
  return path.relative(ROOT, filepath).replace(/\\/g, "/");
}

function safeExec(cmd) {
  try {
    return execSync(cmd, { cwd: ROOT, encoding: "utf-8", timeout: 10000 });
  } catch {
    return "";
  }
}

function grepFiles(dir, extensions, regex) {
  const results = [];
  for (const file of walkDir(dir, extensions)) {
    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (regex.test(lines[i])) {
        results.push({ file: relPath(file), line: i + 1, text: lines[i].trim() });
      }
    }
  }
  return results;
}

function getDirectoryModules(dir) {
  const modules = {};
  try {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory() && !["target", "bin"].includes(entry.name)) {
        const modDir = path.join(dir, entry.name);
        const files = walkDir(modDir, [".rs"]);
        const totalLines = files.reduce((sum, f) => sum + lineCount(f), 0);
        modules[entry.name] = {
          files: files.map((f) => relPath(f)),
          fileCount: files.length,
          totalLines,
        };
      }
    }
  } catch {}
  return modules;
}

function getRecentGitLog(pathFilter, count = 10) {
  return safeExec(
    `git log --oneline -${count} -- "${pathFilter}"`
  ).trim();
}

// ---------------------------------------------------------------------------
// IPC Contract Parsers (from validate-commands.cjs)
// ---------------------------------------------------------------------------

function parseRustCommands() {
  const rustFiles = walkDir(RUST_SRC, [".rs"]);
  const commands = new Map(); // name -> file

  for (const file of rustFiles) {
    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trim().includes("#[tauri::command]")) {
        for (let j = i + 1; j < Math.min(i + 10, lines.length); j++) {
          const fnMatch = lines[j].match(
            /^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/
          );
          if (fnMatch) {
            commands.set(fnMatch[1], relPath(file));
            break;
          }
        }
      }
    }
  }
  return commands;
}

function parseRegisteredCommands() {
  const content = fs.readFileSync(LIB_RS, "utf-8");
  const registered = new Map(); // name -> module path

  const handlerMatch = content.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!handlerMatch) return registered;

  for (const line of handlerMatch[1].split("\n")) {
    const stripped = line.replace(/\/\/.*$/, "");
    for (const entry of stripped.split(",")) {
      const trimmed = entry.trim();
      if (trimmed && trimmed.includes("::")) {
        const parts = trimmed.split("::");
        const fnName = parts[parts.length - 1].trim();
        if (fnName && /^[a-z_][a-z0-9_]*$/.test(fnName)) {
          registered.set(fnName, parts.slice(0, -1).join("::"));
        }
      }
    }
  }
  return registered;
}

function parseTsCommandFunctions() {
  if (!fs.existsSync(COMMANDS_TS)) return new Set();
  const content = fs.readFileSync(COMMANDS_TS, "utf-8");
  const commands = new Set();
  const lines = content.split("\n");

  for (const line of lines) {
    // Match function exports: export async function name(
    const funcMatch = line.match(
      /(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/
    );
    if (funcMatch) {
      commands.add(funcMatch[1]);
    }
    // Match object property: command_name: { params:
    const objMatch = line.match(/^\s+([a-z_][a-z0-9_]*)\s*:\s*\{/);
    if (objMatch) {
      commands.add(objMatch[1]);
    }
  }
  return commands;
}

function findRawInvokes() {
  const tsFiles = walkDir(REACT_SRC, [".ts", ".tsx"]);
  const cmdNorm = path.normalize(COMMANDS_TS);
  const rawInvokes = [];

  for (const file of tsFiles) {
    if (path.normalize(file) === cmdNorm) continue;
    const content = fs.readFileSync(file, "utf-8");
    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (/\binvoke\s*[<(]/.test(lines[i])) {
        rawInvokes.push({
          file: relPath(file),
          line: i + 1,
          text: lines[i].trim(),
        });
      }
    }
  }
  return rawInvokes;
}

// ---------------------------------------------------------------------------
// GENERATOR 1: Topology (for Chief router)
// ---------------------------------------------------------------------------

function generateTopology() {
  const rustFiles = walkDir(RUST_SRC, [".rs"]);
  const tsFiles = walkDir(REACT_SRC, [".ts", ".tsx"]);
  const rustModules = getDirectoryModules(RUST_SRC);
  const registered = parseRegisteredCommands();

  // Count top-level Rust files (not in subdirectories)
  const topLevelRust = rustFiles.filter((f) => {
    const rel = path.relative(RUST_SRC, f);
    return !rel.includes(path.sep);
  });

  // Count component directories
  const componentDir = path.join(REACT_SRC, "components");
  const componentDirs = {};
  try {
    const entries = fs.readdirSync(componentDir, { withFileTypes: true });
    for (const e of entries) {
      if (e.isDirectory()) {
        const files = walkDir(path.join(componentDir, e.name), [".ts", ".tsx"]);
        componentDirs[e.name] = files.length;
      }
    }
  } catch {}

  // Group registered commands by module
  const commandsByModule = {};
  for (const [cmd, mod] of registered) {
    const key = mod.split("::")[0];
    if (!commandsByModule[key]) commandsByModule[key] = [];
    commandsByModule[key].push(cmd);
  }

  let md = `# 4DA System Topology
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Scale
| Metric | Count |
|--------|-------|
| Rust files | ${rustFiles.length} |
| TypeScript files | ${tsFiles.length} |
| Registered IPC commands | ${registered.size} |
| Rust modules (directories) | ${Object.keys(rustModules).length} |
| Frontend component groups | ${Object.keys(componentDirs).length} |

## Rust Module Map
| Module | Files | Lines | Domain |
|--------|-------|-------|--------|
`;

  const domainMap = {
    ace: "Context Engine",
    autophagy: "Content Metabolism",
    db: "Data Layer",
    scoring: "Scoring & ML",
    settings: "Settings",
    sources: "Content Sources",
    source_fetching: "Source Fetching",
    extractors: "File Extractors",
    plugins: "Plugin System",
    query: "Query Engine",
    content_personalization: "Content Personalization",
    decision_advantage: "Decision Intelligence",
    stacks: "Stack Intelligence",
    suns: "STREETS Health",
    taste_test: "Taste Test Calibration",
    void_engine: "Void Engine",
  };

  for (const [mod, data] of Object.entries(rustModules).sort()) {
    const domain = domainMap[mod] || mod;
    md += `| \`${mod}/\` | ${data.fileCount} | ${data.totalLines} | ${domain} |\n`;
  }

  md += `| _(top-level .rs)_ | ${topLevelRust.length} | ${topLevelRust.reduce((s, f) => s + lineCount(f), 0)} | Commands & Core |\n`;

  md += `\n## IPC Commands by Domain\n`;
  for (const [mod, cmds] of Object.entries(commandsByModule).sort()) {
    md += `- **${mod}** (${cmds.length}): ${cmds.slice(0, 5).join(", ")}${cmds.length > 5 ? ` ... +${cmds.length - 5} more` : ""}\n`;
  }

  md += `\n## Frontend Component Groups\n| Group | Files |\n|-------|-------|\n`;
  for (const [dir, count] of Object.entries(componentDirs).sort()) {
    md += `| \`${dir}/\` | ${count} |\n`;
  }

  md += `\n## Domain → Expert Routing\n`;
  md += `| Issue Area | Primary Expert | Backup Expert |\n`;
  md += `|------------|---------------|---------------|\n`;
  md += `| Rust compilation, lifetimes, async | Rust Systems | — |\n`;
  md += `| React components, hooks, state | React UI | — |\n`;
  md += `| SQLite, migrations, sqlite-vec | Data Layer | Rust Systems |\n`;
  md += `| IPC commands, invoke, serialization | IPC Bridge | Rust Systems + React UI |\n`;
  md += `| PASIFA scoring, embeddings, relevance | Scoring & ML | Data Layer |\n`;
  md += `| Security, privacy, invariants | Security | All |\n`;
  md += `| Source fetching, API failures | Rust Systems | IPC Bridge |\n`;
  md += `| UI layout, design system, a11y | React UI | — |\n`;
  md += `| Performance, memory, latency | Rust Systems | React UI |\n`;

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 2: Rust Systems
// ---------------------------------------------------------------------------

function generateRustSystems() {
  const modules = getDirectoryModules(RUST_SRC);
  const allRustFiles = walkDir(RUST_SRC, [".rs"]);

  // Extract key public items
  const traits = grepFiles(RUST_SRC, [".rs"], /^\s*pub\s+(?:async\s+)?trait\s+/);
  const structs = grepFiles(RUST_SRC, [".rs"], /^\s*pub\s+struct\s+/);
  const enums = grepFiles(RUST_SRC, [".rs"], /^\s*pub\s+enum\s+/);
  const errorTypes = grepFiles(RUST_SRC, [".rs"], /derive.*thiserror|#\[error\(/);

  // Top-level files with sizes (the command files)
  const topLevel = allRustFiles
    .filter((f) => !path.relative(RUST_SRC, f).includes(path.sep))
    .map((f) => ({
      name: path.basename(f),
      lines: lineCount(f),
      rel: relPath(f),
    }))
    .sort((a, b) => b.lines - a.lines);

  const recentGit = getRecentGitLog("src-tauri/src/");

  // Cargo dependencies
  let deps = [];
  try {
    const cargo = fs.readFileSync(
      path.join(ROOT, "src-tauri", "Cargo.toml"),
      "utf-8"
    );
    const depMatch = cargo.match(/\[dependencies\]([\s\S]*?)(?:\n\[|\n*$)/);
    if (depMatch) {
      deps = depMatch[1]
        .split("\n")
        .filter((l) => l.match(/^[a-z]/))
        .map((l) => l.split("=")[0].trim())
        .slice(0, 40);
    }
  } catch {}

  let md = `# Rust Systems Knowledge
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Module Inventory
| Module | Files | Total Lines | Purpose |
|--------|-------|-------------|---------|
`;

  for (const [mod, data] of Object.entries(modules).sort()) {
    md += `| \`${mod}/\` | ${data.fileCount} | ${data.totalLines} | |\n`;
  }

  md += `\n## Top-Level Files (by size)\n| File | Lines |\n|------|-------|\n`;
  for (const f of topLevel.slice(0, 30)) {
    md += `| \`${f.name}\` | ${f.lines} |\n`;
  }

  md += `\n## Key Traits (${traits.length})\n`;
  for (const t of traits.slice(0, 20)) {
    md += `- \`${t.file}:${t.line}\` — ${t.text.replace(/^\s*pub\s+/, "")}\n`;
  }

  md += `\n## Key Structs (${structs.length} total, showing top 30)\n`;
  for (const s of structs.slice(0, 30)) {
    const name = s.text.match(/struct\s+(\w+)/)?.[1] || s.text;
    md += `- \`${s.file}:${s.line}\` — ${name}\n`;
  }

  md += `\n## Error Types (${errorTypes.length})\n`;
  for (const e of errorTypes.slice(0, 15)) {
    md += `- \`${e.file}:${e.line}\` — ${e.text}\n`;
  }

  md += `\n## Key Dependencies\n${deps.map((d) => `- ${d}`).join("\n")}\n`;

  md += `\n## Critical Patterns\n`;
  md += `- **MutexGuard across await:** \`MutexGuard<SourceRegistry>\` is NOT Send. Never hold across .await.\n`;
  md += `- **Error handling:** Use \`thiserror\` for types, \`anyhow\` for app errors. ResultExt in \`error.rs\` for .context()/.with_context().\n`;
  md += `- **No unwrap/panic:** Use graceful fallbacks in production code.\n`;
  md += `- **sqlite-vec KNN:** Use \`k = ?\` in WHERE, NOT \`LIMIT\` at end.\n`;

  md += `\n## Recent Changes\n\`\`\`\n${recentGit || "(no recent changes)"}\n\`\`\`\n`;

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 3: React UI
// ---------------------------------------------------------------------------

function generateReactUI() {
  const componentDir = path.join(REACT_SRC, "components");
  const hooksDir = path.join(REACT_SRC, "hooks");
  const storeDir = path.join(REACT_SRC, "store");
  const typesDir = path.join(REACT_SRC, "types");
  const localesDir = path.join(REACT_SRC, "locales");

  // Components
  const componentGroups = {};
  const topLevelComponents = [];
  try {
    const entries = fs.readdirSync(componentDir, { withFileTypes: true });
    for (const e of entries) {
      const full = path.join(componentDir, e.name);
      if (e.isDirectory()) {
        const files = walkDir(full, [".tsx", ".ts"]).map((f) => ({
          name: path.basename(f),
          lines: lineCount(f),
        }));
        componentGroups[e.name] = files;
      } else if (e.name.endsWith(".tsx") || e.name.endsWith(".ts")) {
        topLevelComponents.push({
          name: e.name,
          lines: lineCount(full),
        });
      }
    }
  } catch {}

  // Hooks
  const hooks = walkDir(hooksDir, [".ts", ".tsx"])
    .filter((f) => !f.includes("__tests__"))
    .map((f) => ({
      name: path.basename(f),
      lines: lineCount(f),
    }));

  // Store
  const storeFiles = walkDir(storeDir, [".ts"])
    .filter((f) => !f.includes("__tests__"))
    .map((f) => ({
      name: path.basename(f),
      lines: lineCount(f),
    }));

  // Types
  const typeFiles = walkDir(typesDir, [".ts"]).map((f) => ({
    name: path.basename(f),
    lines: lineCount(f),
  }));

  // i18n coverage
  const locales = [];
  try {
    const entries = fs.readdirSync(localesDir, { withFileTypes: true });
    for (const e of entries) {
      if (e.isDirectory()) {
        const files = walkDir(path.join(localesDir, e.name), [".json"]);
        locales.push({ locale: e.name, fileCount: files.length });
      }
    }
  } catch {}

  const recentGit = getRecentGitLog("src/");

  let md = `# React UI Knowledge
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Component Groups
| Group | Files | Largest File |
|-------|-------|-------------|
`;

  for (const [group, files] of Object.entries(componentGroups).sort()) {
    const largest = files.sort((a, b) => b.lines - a.lines)[0];
    md += `| \`${group}/\` | ${files.length} | ${largest ? `${largest.name} (${largest.lines}L)` : "—"} |\n`;
  }

  md += `\n## Top-Level Components (${topLevelComponents.length})\n`;
  const topSorted = topLevelComponents.sort((a, b) => b.lines - a.lines);
  for (const c of topSorted.slice(0, 20)) {
    md += `- \`${c.name}\` (${c.lines}L)\n`;
  }

  md += `\n## Hooks (${hooks.length})\n`;
  for (const h of hooks) {
    md += `- \`${h.name}\` (${h.lines}L)\n`;
  }

  md += `\n## Store (${storeFiles.length})\n`;
  for (const s of storeFiles) {
    md += `- \`${s.name}\` (${s.lines}L)\n`;
  }

  md += `\n## Types (${typeFiles.length})\n`;
  for (const t of typeFiles) {
    md += `- \`${t.name}\` (${t.lines}L)\n`;
  }

  md += `\n## i18n Coverage\n| Locale | Files |\n|--------|-------|\n`;
  for (const l of locales) {
    md += `| ${l.locale} | ${l.fileCount} |\n`;
  }

  md += `\n## Design System Tokens\n`;
  md += `- Background: #0A0A0A / #141414 / #1F1F1F\n`;
  md += `- Text: #FFFFFF / #A0A0A0 / #8A8A8A\n`;
  md += `- Accent: #FFFFFF / #D4AF37 / #2A2A2A\n`;
  md += `- Fonts: Inter (UI), JetBrains Mono (code)\n`;

  md += `\n## Critical Patterns\n`;
  md += `- All IPC calls go through \`src/lib/commands.ts\` — never raw \`invoke()\`\n`;
  md += `- Error boundaries required for all component trees\n`;
  md += `- i18n: all user-facing strings via \`useTranslation()\`\n`;
  md += `- File limits: .tsx warn at 300L, error at 450L\n`;

  md += `\n## Recent Changes\n\`\`\`\n${recentGit || "(no recent changes)"}\n\`\`\`\n`;

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 4: IPC Contracts (the crown jewel)
// ---------------------------------------------------------------------------

function generateIPCContracts() {
  const rustCommands = parseRustCommands();
  const registered = parseRegisteredCommands();
  const tsFunctions = parseTsCommandFunctions();
  const rawInvokes = findRawInvokes();

  // Cross-reference analysis
  const registeredNotTs = [...registered.keys()]
    .filter((c) => !tsFunctions.has(c))
    .sort();
  const tsNotRegistered = [...tsFunctions]
    .filter((c) => !registered.has(c))
    .sort();
  const definedNotRegistered = [...rustCommands.keys()]
    .filter((c) => !registered.has(c))
    .sort();
  const healthy = [...registered.keys()]
    .filter((c) => tsFunctions.has(c))
    .sort();

  const issueCount =
    registeredNotTs.length + tsNotRegistered.length + definedNotRegistered.length;

  let md = `# IPC Contract Map
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Health Summary
| Metric | Count | Status |
|--------|-------|--------|
| Rust \`#[tauri::command]\` definitions | ${rustCommands.size} | — |
| \`generate_handler![]\` registrations | ${registered.size} | — |
| \`commands.ts\` typed functions | ${tsFunctions.size} | — |
| Raw \`invoke()\` bypasses | ${rawInvokes.length} | ${rawInvokes.length > 10 ? "WARNING" : "OK"} |
| Healthy (registered + typed) | ${healthy.length} | — |
| **Contract issues** | **${issueCount}** | **${issueCount === 0 ? "CLEAN" : "NEEDS ATTENTION"}** |

`;

  if (registeredNotTs.length > 0) {
    md += `## GHOST RISK: Registered but NO TypeScript Binding (${registeredNotTs.length})\nThese commands are registered in Rust but have no typed function in commands.ts.\nFrontend code calling these will silently fail.\n`;
    for (const cmd of registeredNotTs) {
      md += `- \`${cmd}\` (module: ${registered.get(cmd)})\n`;
    }
    md += `\n`;
  }

  if (tsNotRegistered.length > 0) {
    md += `## DEAD CODE: In commands.ts but NOT Registered (${tsNotRegistered.length})\nThese TypeScript functions have no registered Rust handler — calls will always fail.\n`;
    for (const cmd of tsNotRegistered) {
      md += `- \`${cmd}\`\n`;
    }
    md += `\n`;
  }

  if (definedNotRegistered.length > 0) {
    md += `## UNUSED: Rust #[tauri::command] but NOT Registered (${definedNotRegistered.length})\nThese functions have the command attribute but are not in generate_handler![].\n`;
    for (const cmd of definedNotRegistered) {
      md += `- \`${cmd}\` in \`${rustCommands.get(cmd)}\`\n`;
    }
    md += `\n`;
  }

  if (rawInvokes.length > 0) {
    md += `## Raw invoke() Calls Bypassing Typed Layer (${rawInvokes.length})\n`;
    // Group by file
    const byFile = {};
    for (const inv of rawInvokes) {
      if (!byFile[inv.file]) byFile[inv.file] = [];
      byFile[inv.file].push(inv);
    }
    for (const [file, invs] of Object.entries(byFile)) {
      md += `### \`${file}\`\n`;
      for (const inv of invs) {
        md += `- L${inv.line}: \`${inv.text.slice(0, 100)}\`\n`;
      }
    }
    md += `\n`;
  }

  md += `## Full Command Registry (${registered.size})\n`;
  md += `| Command | Rust Module | In commands.ts |\n`;
  md += `|---------|-------------|---------------|\n`;

  const allCmds = [...registered.keys()].sort();
  for (const cmd of allCmds) {
    const inTs = tsFunctions.has(cmd) ? "yes" : "**NO**";
    md += `| \`${cmd}\` | ${registered.get(cmd)} | ${inTs} |\n`;
  }

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 5: Data Layer
// ---------------------------------------------------------------------------

function generateDataLayer() {
  const dbDir = path.join(RUST_SRC, "db");
  const dbFiles = walkDir(dbDir, [".rs"]).map((f) => ({
    name: path.basename(f),
    lines: lineCount(f),
    rel: relPath(f),
  }));

  // Schema patterns
  const createTables = grepFiles(RUST_SRC, [".rs"], /CREATE\s+TABLE/i);
  const alterTables = grepFiles(RUST_SRC, [".rs"], /ALTER\s+TABLE/i);

  // Transaction usage
  const transactions = grepFiles(RUST_SRC, [".rs"], /\.transaction\s*\(|Transaction::new|begin_transaction/);

  // sqlite-vec patterns
  const vecQueries = grepFiles(RUST_SRC, [".rs"], /vec_search|sqlite_vec|embedding.*distance|knn|vec0/i);
  const kPatterns = grepFiles(RUST_SRC, [".rs"], /k\s*=\s*\?/);

  const recentGit = getRecentGitLog("src-tauri/src/db/");

  let md = `# Data Layer Knowledge
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## DB Module Files
| File | Lines |
|------|-------|
`;

  for (const f of dbFiles.sort((a, b) => b.lines - a.lines)) {
    md += `| \`${f.name}\` | ${f.lines} |\n`;
  }

  md += `\n## Schema Definitions (CREATE TABLE: ${createTables.length})\n`;
  for (const ct of createTables) {
    md += `- \`${ct.file}:${ct.line}\` — ${ct.text.slice(0, 100)}\n`;
  }

  md += `\n## Migrations (ALTER TABLE: ${alterTables.length})\n`;
  for (const at of alterTables) {
    md += `- \`${at.file}:${at.line}\` — ${at.text.slice(0, 100)}\n`;
  }

  md += `\n## Transaction Usage (${transactions.length} call sites)\n`;
  for (const t of transactions.slice(0, 20)) {
    md += `- \`${t.file}:${t.line}\`\n`;
  }

  md += `\n## sqlite-vec / Vector Search (${vecQueries.length} references)\n`;
  for (const v of vecQueries.slice(0, 15)) {
    md += `- \`${v.file}:${v.line}\` — ${v.text.slice(0, 100)}\n`;
  }

  md += `\n## KNN k= Pattern Usage (${kPatterns.length})\n`;
  for (const k of kPatterns) {
    md += `- \`${k.file}:${k.line}\` — ${k.text.slice(0, 80)}\n`;
  }

  md += `\n## Critical Gotchas\n`;
  md += `- **sqlite-vec KNN:** \`k = ?\` goes in WHERE clause, NOT \`LIMIT\` at end\n`;
  md += `- **All DB writes** must be wrapped in transactions\n`;
  md += `- **Schema changes** require migration entries in \`db/migrations.rs\`\n`;
  md += `- **Embedding dimensions** must match across system (check embeddings.rs)\n`;

  md += `\n## Recent Changes\n\`\`\`\n${recentGit || "(no recent changes)"}\n\`\`\`\n`;

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 6: Scoring & ML
// ---------------------------------------------------------------------------

function generateScoringML() {
  const scoringDir = path.join(RUST_SRC, "scoring");
  const scoringFiles = walkDir(scoringDir, [".rs"]).map((f) => ({
    name: path.basename(f),
    lines: lineCount(f),
    rel: relPath(f),
  }));

  // ACE files
  const aceDir = path.join(RUST_SRC, "ace");
  const aceFiles = walkDir(aceDir, [".rs"]).map((f) => ({
    name: path.basename(f),
    lines: lineCount(f),
    rel: relPath(f),
  }));

  // Embedding references
  const embeddingFile = path.join(RUST_SRC, "embeddings.rs");
  const embeddingLines = lineCount(embeddingFile);

  // Scoring-related patterns
  const scoreCalcs = grepFiles(scoringDir, [".rs"], /fn\s+(?:calculate|compute|score)/);
  const thresholds = grepFiles(RUST_SRC, [".rs"], /threshold|THRESHOLD/);

  // Taste test
  const tasteDir = path.join(RUST_SRC, "taste_test");
  const tasteFiles = walkDir(tasteDir, [".rs"]).map((f) => ({
    name: path.basename(f),
    lines: lineCount(f),
  }));

  const recentGit = getRecentGitLog("src-tauri/src/scoring/");

  let md = `# Scoring & ML Knowledge
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Scoring Module (${scoringFiles.length} files)
| File | Lines |
|------|-------|
`;

  for (const f of scoringFiles.sort((a, b) => b.lines - a.lines)) {
    md += `| \`${f.name}\` | ${f.lines} |\n`;
  }

  md += `\n## ACE — Autonomous Context Engine (${aceFiles.length} files)\n`;
  md += `| File | Lines |\n|------|-------|\n`;
  for (const f of aceFiles.sort((a, b) => b.lines - a.lines)) {
    md += `| \`${f.name}\` | ${f.lines} |\n`;
  }

  md += `\n## Embeddings\n`;
  md += `- Main file: \`src-tauri/src/embeddings.rs\` (${embeddingLines}L)\n`;
  md += `- Provider: Local via Ollama, fallback to zero vectors\n`;

  md += `\n## Taste Test Calibration (${tasteFiles.length} files)\n`;
  for (const f of tasteFiles) {
    md += `- \`${f.name}\` (${f.lines}L)\n`;
  }

  md += `\n## Scoring Functions (${scoreCalcs.length})\n`;
  for (const sc of scoreCalcs.slice(0, 15)) {
    md += `- \`${sc.file}:${sc.line}\` — ${sc.text.slice(0, 80)}\n`;
  }

  md += `\n## Threshold References (${thresholds.length})\n`;
  for (const t of thresholds.slice(0, 10)) {
    md += `- \`${t.file}:${t.line}\` — ${t.text.slice(0, 80)}\n`;
  }

  md += `\n## Pipeline Architecture\n`;
  md += `\`\`\`\n`;
  md += `Content → Embedding → Semantic Match → PASIFA Score → Threshold → Display\n`;
  md += `   │          │            │               │              │\n`;
  md += `   └─ Parse   └─ Ollama    └─ sqlite-vec   └─ Confidence  └─ Auto-tune\n`;
  md += `      Clean      or zero     KNN search      Weighted       Dynamic\n`;
  md += `\`\`\`\n`;

  md += `\n## Critical Gotchas\n`;
  md += `- **Embedding dimensions** must match between generation and search\n`;
  md += `- **PASIFA** is confidence-weighted with auto-tuning threshold\n`;
  md += `- **preprocess_content()** in utils.rs — strip HTML, decode entities, cap 2000 chars\n`;
  md += `- **Near-misses**: extract_near_misses() populates when <3 relevant results\n`;

  md += `\n## Recent Changes\n\`\`\`\n${recentGit || "(no recent changes)"}\n\`\`\`\n`;

  return md;
}

// ---------------------------------------------------------------------------
// GENERATOR 7: Security Surface
// ---------------------------------------------------------------------------

function generateSecuritySurface() {
  // Key security patterns to scan for
  const apiKeyLogs = grepFiles(RUST_SRC, [".rs"], /(?:log|debug|info|warn|error|println|eprintln).*(?:api_key|secret|token|password)/i);
  const unwraps = grepFiles(RUST_SRC, [".rs"], /\.unwrap\(\)/);
  const panics = grepFiles(RUST_SRC, [".rs"], /panic!\(|todo!\(|unimplemented!\(/);
  const unsafeBlocks = grepFiles(RUST_SRC, [".rs"], /unsafe\s*\{/);
  const sqlInjection = grepFiles(RUST_SRC, [".rs"], /format!\(.*(?:SELECT|INSERT|UPDATE|DELETE|DROP)/i);
  const hardcodedSecrets = grepFiles(RUST_SRC, [".rs"], /(?:api_key|secret|password|token)\s*=\s*"/i);

  // Frontend security
  const dangerousHTML = grepFiles(REACT_SRC, [".tsx"], /dangerouslySetInnerHTML/);
  const localStorage = grepFiles(REACT_SRC, [".ts", ".tsx"], /localStorage\./);

  // Privacy checks
  const networkCalls = grepFiles(RUST_SRC, [".rs"], /reqwest|hyper|surf|ureq/);
  const telemetry = grepFiles(RUST_SRC, [".rs"], /telemetry|analytics|tracking/i);

  let md = `# Security Surface Analysis
> Auto-generated ${TIMESTAMP} — DO NOT EDIT

## Invariant Compliance
| Check | Count | Severity | Status |
|-------|-------|----------|--------|
| API key in logs | ${apiKeyLogs.length} | CRITICAL | ${apiKeyLogs.length === 0 ? "PASS" : "**FAIL**"} |
| .unwrap() usage | ${unwraps.length} | High | ${unwraps.length < 10 ? "OK" : "REVIEW"} |
| panic!/todo!/unimplemented! | ${panics.length} | High | ${panics.length < 5 ? "OK" : "REVIEW"} |
| unsafe blocks | ${unsafeBlocks.length} | Medium | ${unsafeBlocks.length === 0 ? "PASS" : "REVIEW"} |
| SQL string formatting | ${sqlInjection.length} | CRITICAL | ${sqlInjection.length === 0 ? "PASS" : "**FAIL**"} |
| Hardcoded secrets | ${hardcodedSecrets.length} | CRITICAL | ${hardcodedSecrets.length === 0 ? "PASS" : "**FAIL**"} |
| dangerouslySetInnerHTML | ${dangerousHTML.length} | High | ${dangerousHTML.length === 0 ? "PASS" : "REVIEW"} |
| localStorage usage | ${localStorage.length} | Low | INFO |

`;

  if (apiKeyLogs.length > 0) {
    md += `## API Key Logging Violations\n`;
    for (const v of apiKeyLogs) {
      md += `- \`${v.file}:${v.line}\` — ${v.text.slice(0, 100)}\n`;
    }
    md += `\n`;
  }

  if (unwraps.length > 0) {
    md += `## .unwrap() Usage (${unwraps.length} — review each)\n`;
    // Group by file
    const byFile = {};
    for (const u of unwraps) {
      if (!byFile[u.file]) byFile[u.file] = 0;
      byFile[u.file]++;
    }
    for (const [file, count] of Object.entries(byFile).sort((a, b) => b[1] - a[1]).slice(0, 15)) {
      md += `- \`${file}\` — ${count} occurrences\n`;
    }
    md += `\n`;
  }

  if (panics.length > 0) {
    md += `## Panic Points (${panics.length})\n`;
    for (const p of panics.slice(0, 10)) {
      md += `- \`${p.file}:${p.line}\` — ${p.text.slice(0, 80)}\n`;
    }
    md += `\n`;
  }

  if (sqlInjection.length > 0) {
    md += `## Potential SQL Injection Vectors\n`;
    for (const s of sqlInjection) {
      md += `- \`${s.file}:${s.line}\` — ${s.text.slice(0, 100)}\n`;
    }
    md += `\n`;
  }

  md += `## Network Surface (${networkCalls.length} references)\n`;
  md += `All network calls should respect privacy — raw user data never leaves the machine.\n`;
  const netByFile = {};
  for (const n of networkCalls) {
    if (!netByFile[n.file]) netByFile[n.file] = 0;
    netByFile[n.file]++;
  }
  for (const [file, count] of Object.entries(netByFile)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)) {
    md += `- \`${file}\` — ${count} references\n`;
  }

  md += `\n## Privacy Boundaries\n`;
  md += `- All data processed locally — never sent to external servers\n`;
  md += `- BYOK: API keys stored in local settings.json (gitignored)\n`;
  md += `- Telemetry is local-only (never leaves machine)\n`;
  md += `- Team Relay uses E2E encryption (XChaCha20Poly1305)\n`;

  md += `\n## Key Invariants (from .ai/INVARIANTS.md)\n`;
  md += `- INV-001: API keys never logged or in error messages (CRITICAL)\n`;
  md += `- INV-002: All DB writes wrapped in transactions (HIGH)\n`;
  md += `- INV-003: All Tauri commands return Result types (HIGH)\n`;
  md += `- INV-004: All user data stays local (CRITICAL)\n`;
  md += `- INV-005: Embedding dimensions match across system (MEDIUM)\n`;

  return md;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  console.log(`[generate-knowledge] Scanning codebase at ${TIMESTAMP}...`);

  const generators = {
    topology: generateTopology,
    "rust-systems": generateRustSystems,
    "react-ui": generateReactUI,
    "ipc-contracts": generateIPCContracts,
    "data-layer": generateDataLayer,
    "scoring-ml": generateScoringML,
    "security-surface": generateSecuritySurface,
  };

  let totalSize = 0;
  for (const [name, fn] of Object.entries(generators)) {
    try {
      const content = fn();
      const outPath = path.join(KNOWLEDGE_DIR, `${name}.md`);
      fs.writeFileSync(outPath, content, "utf-8");
      const size = Buffer.byteLength(content, "utf-8");
      totalSize += size;
      console.log(`  ✓ ${name}.md (${(size / 1024).toFixed(1)}KB)`);
    } catch (err) {
      console.error(`  ✗ ${name}.md — ${err.message}`);
    }
  }

  console.log(
    `\n[generate-knowledge] Done. ${Object.keys(generators).length} manifests, ${(totalSize / 1024).toFixed(1)}KB total.`
  );
}

main();
