#!/usr/bin/env node
/**
 * Add SPDX-License-Identifier header to every source file that lacks one.
 *
 * Idempotent: files already starting with the SPDX line are skipped.
 * Safe: only touches .rs / .ts / .tsx / .cjs / .mjs in the known source dirs.
 *
 * Run: `node scripts/add-spdx-headers.cjs`
 * Dry: `node scripts/add-spdx-headers.cjs --dry`
 */

const fs = require('fs');
const path = require('path');

const SPDX_LINE = '// SPDX-License-Identifier: FSL-1.1-Apache-2.0';
const SPDX_MATCH = /^\/\/\s*SPDX-License-Identifier:/;

const DIRS = [
  'src',
  'src-tauri/src',
  'mcp-4da-server/src',
  'mcp-memory-server/src',
  'mcp-streets-server/src',
];

const EXTS = new Set(['.rs', '.ts', '.tsx', '.cjs', '.mjs']);

const EXCLUDES = [
  /[\/\\]node_modules[\/\\]/,
  /[\/\\]target[\/\\]/,
  /[\/\\]dist[\/\\]/,
  /\.d\.ts$/, // vite-env.d.ts etc — tooling declarations
];

const args = new Set(process.argv.slice(2));
const dryRun = args.has('--dry');

let added = 0;
let skipped = 0;
let scanned = 0;

function walk(dir) {
  if (!fs.existsSync(dir)) return;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (EXCLUDES.some((re) => re.test(full))) continue;
    if (entry.isDirectory()) {
      walk(full);
    } else if (entry.isFile() && EXTS.has(path.extname(entry.name))) {
      processFile(full);
    }
  }
}

function processFile(file) {
  scanned++;
  const content = fs.readFileSync(file, 'utf8');
  const firstLine = content.split(/\r?\n/, 1)[0] || '';
  if (SPDX_MATCH.test(firstLine)) {
    skipped++;
    return;
  }
  // Skip files starting with a shebang — SPDX goes after the shebang.
  let prefix = '';
  let body = content;
  if (body.startsWith('#!')) {
    const idx = body.indexOf('\n');
    if (idx >= 0) {
      prefix = body.slice(0, idx + 1);
      body = body.slice(idx + 1);
    }
  }
  const newContent = `${prefix}${SPDX_LINE}\n${body}`;
  if (dryRun) {
    console.log(`[dry] would add SPDX to ${file}`);
  } else {
    fs.writeFileSync(file, newContent, 'utf8');
  }
  added++;
}

for (const dir of DIRS) {
  walk(path.resolve(dir));
}

console.log(
  `SPDX header pass complete: scanned ${scanned}, added ${added}, already present ${skipped}${dryRun ? ' (dry run)' : ''}`,
);
