#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Keeps the in-process embedding model out of RELEASE bundles (AD-054
// decision 7), and reports what the bundled `models` resource weighs.
//
// The dependency engine needs no embeddings; only opt-in interests and
// natural-language search use them. The nomic fp16 weights are 261 MB raw
// (~234 MB LZMA-compressed), most of every installer, so the app now
// downloads them on first use (src-tauri/src/embeddings_providers/model_store.rs:
// same pinned revision, every file SHA-256 pinned, resumable).
//
// Same shape as release-ocr-models.cjs (#900), for the same reason: tauri.conf.json
// keeps mapping the whole `src-tauri/models` directory, because tauri-build checks
// at COMPILE time that every bundle resource exists, and narrowing the mapping
// would break dev and CI builds. Dev builds read src-tauri/models in place and
// are untouched; the CI real-embedding gate still fetches the model itself.
// `bundle:resources` (tauri's beforeBuildCommand) no longer fetches the model,
// and this step removes any copy already in the tree before tauri bundles it.
//
// Usage (release.yml, after `pnpm install`, before the tauri build):
//   node scripts/release-embedding-model.cjs [--dry-run]   # strip (default)
//   node scripts/release-embedding-model.cjs --bundle      # ship it: fetch + verify
//   --models-dir <dir> points it at another tree (tests).

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const MODELS_DIR = path.resolve(__dirname, '..', 'src-tauri', 'models');
const EMBEDDINGS_SUBDIR = 'embeddings';

/** Every file under `dir` as `{ rel, size }`, `rel` using forward slashes. */
function listFiles(dir, base = dir) {
  if (!fs.existsSync(dir)) return [];
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listFiles(full, base));
    else if (entry.isFile()) {
      out.push({ rel: path.relative(base, full).split(path.sep).join('/'), size: fs.statSync(full).size });
    }
  }
  return out.sort((a, b) => a.rel.localeCompare(b.rel));
}

/** Whether a models-relative path belongs to the embedding model. */
function isEmbeddingFile(rel) {
  return rel === EMBEDDINGS_SUBDIR || rel.startsWith(`${EMBEDDINGS_SUBDIR}/`);
}

/** Split the bundled resource into what ships and what this step removes. */
function planStrip(modelsDir) {
  const files = listFiles(modelsDir);
  const strip = files.filter((f) => isEmbeddingFile(f.rel));
  const keep = files.filter((f) => !isEmbeddingFile(f.rel));
  const sum = (xs) => xs.reduce((n, f) => n + f.size, 0);
  return { keep, strip, keepBytes: sum(keep), stripBytes: sum(strip) };
}

const mb = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`;

/** The dry-run size report, one line per file. */
function formatReport(plan) {
  const lines = ['[release-embedding-model] bundled `models` resource (raw sizes):'];
  for (const f of plan.keep) lines.push(`  ship   ${mb(f.size).padStart(9)}  ${f.rel}`);
  for (const f of plan.strip) lines.push(`  strip  ${mb(f.size).padStart(9)}  ${f.rel}`);
  lines.push(
    `  before ${mb(plan.keepBytes + plan.stripBytes)}, after ${mb(plan.keepBytes)} ` +
      `(${mb(plan.stripBytes)} of embedding model removed; the app downloads it on first use)`,
  );
  return lines.join('\n');
}

function parseArgs(argv) {
  const i = argv.indexOf('--models-dir');
  return {
    dryRun: argv.includes('--dry-run'),
    bundle: argv.includes('--bundle'),
    modelsDir: i >= 0 && argv[i + 1] ? path.resolve(argv[i + 1]) : MODELS_DIR,
  };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const modelsDir = args.modelsDir;
  if (args.bundle) {
    console.log('[release-embedding-model] --bundle: this release ships the model — fetching the pinned copy.');
    if (args.dryRun) return;
    const r = spawnSync(process.execPath, [path.join(__dirname, 'download-embedding-model.cjs')], {
      stdio: 'inherit',
    });
    process.exit(r.status ?? 1);
  }

  const plan = planStrip(modelsDir);
  console.log(formatReport(plan));
  if (args.dryRun) return;

  fs.rmSync(path.join(modelsDir, EMBEDDINGS_SUBDIR), { recursive: true, force: true });
  const left = planStrip(modelsDir).strip;
  if (left.length) {
    console.error(`[release-embedding-model] could not remove: ${left.map((f) => f.rel).join(', ')}`);
    process.exit(1);
  }
  console.log(`[release-embedding-model] removed ${plan.strip.length} file(s); the embedding model is not in this bundle.`);
}

if (require.main === module) {
  main();
}

module.exports = { listFiles, isEmbeddingFile, planStrip, formatReport, parseArgs, MODELS_DIR };
