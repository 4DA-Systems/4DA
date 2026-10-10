#!/usr/bin/env node
// Decides whether the OCR models ship in a RELEASE bundle, and makes the
// bundle match that decision.
//
// tauri.conf.json bundles the whole `src-tauri/models` directory (ONNX runtime
// + embedding model + the two OCR `.rten` files). The OCR code is entirely
// `#[cfg(feature = "ocr")]` and the `ocr` cargo feature is OFF in release
// builds (Cargo.toml default features; release.yml passes no `--features`), so
// the 12 MB of `.rten` weights shipped in every installer with nothing able to
// load them (audit 2026-10-09).
//
// The directory mapping itself stays as it is on purpose: tauri-build checks
// at COMPILE time that every bundle resource exists, so narrowing the mapping
// to `models/ort` + `models/embeddings` would break every dev and CI build
// that has not downloaded the ~125 MB runtime + model (only the release runs
// `bundle:resources`). Dev and `--features ocr` use is unaffected: those read
// `src-tauri/models` from the source tree (`RuntimePaths::ocr_models_dir`),
// which the `pnpm install` postinstall still provisions.
//
// So the decision is made where the release is built:
//   * `ocr` reachable from the release feature set -> fetch + verify the
//     pinned models (`fetch-ocr-models.cjs --require`), exactly as before;
//   * otherwise -> remove the `.rten` files from `src-tauri/models` before
//     tauri bundles it, and fail if any remain.
//
// Usage (release.yml, after `pnpm install`, before the tauri build):
//   node scripts/release-ocr-models.cjs [--features "a,b"] [--dry-run]

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const { MODELS, MODELS_DIR } = require('./fetch-ocr-models.cjs');

const CARGO_TOML = path.join(__dirname, '..', 'src-tauri', 'Cargo.toml');

/**
 * The `[features]` table of a Cargo.toml as `{ name: [entries] }`. Handles the
 * single-line array form this repo uses (`name = ["a", "dep:b"]`), trailing
 * comments included; anything else in the section is ignored.
 */
function parseFeatures(toml) {
  const features = {};
  let inFeatures = false;
  for (const raw of toml.split(/\r?\n/)) {
    const line = raw.replace(/#.*$/, '').trim();
    if (/^\[.*\]$/.test(line)) {
      inFeatures = line === '[features]';
      continue;
    }
    if (!inFeatures || !line) continue;
    const m = line.match(/^([A-Za-z0-9_-]+)\s*=\s*\[(.*)\]$/);
    if (!m) continue;
    features[m[1]] = [...m[2].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
  }
  return features;
}

/**
 * Every feature enabled by `requested` (plus `default` unless disabled),
 * following feature-to-feature references transitively. `dep:` entries and
 * `crate/feature` entries are not features of this crate and are skipped.
 */
function enabledFeatures(features, requested = [], { noDefault = false } = {}) {
  const enabled = new Set();
  const queue = [...requested, ...(noDefault ? [] : ['default'])];
  while (queue.length) {
    const f = queue.shift();
    if (enabled.has(f)) continue;
    enabled.add(f);
    for (const entry of features[f] ?? []) {
      if (entry.startsWith('dep:') || entry.includes('/')) continue;
      queue.push(entry);
    }
  }
  return enabled;
}

/** Whether the release build compiles the OCR extractor. */
function ocrShipped(toml, requested = [], opts = {}) {
  return enabledFeatures(parseFeatures(toml), requested, opts).has('ocr');
}

/** The OCR model files currently present in `dir`. */
function presentModels(dir) {
  return MODELS.map((m) => path.join(dir, m.file)).filter((p) => fs.existsSync(p));
}

function parseArgs(argv) {
  const out = { features: [], dryRun: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--dry-run') out.dryRun = true;
    else if (argv[i] === '--features') {
      out.features = (argv[++i] ?? '').split(/[,\s]+/).filter(Boolean);
    }
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const toml = fs.readFileSync(CARGO_TOML, 'utf8');
  if (ocrShipped(toml, args.features)) {
    console.log('[release-ocr-models] `ocr` is enabled for this release — fetching the pinned models.');
    if (args.dryRun) return;
    const r = spawnSync(process.execPath, [path.join(__dirname, 'fetch-ocr-models.cjs'), '--require'], {
      stdio: 'inherit',
    });
    process.exit(r.status ?? 1);
  }

  const present = presentModels(MODELS_DIR);
  const bytes = present.reduce((n, p) => n + fs.statSync(p).size, 0);
  console.log(
    `[release-ocr-models] \`ocr\` is OFF in this release — excluding ${present.length} OCR model file(s) ` +
      `(${(bytes / 1024 / 1024).toFixed(1)} MB) from the bundle.`,
  );
  if (args.dryRun) {
    for (const p of present) console.log(`[release-ocr-models]   would remove ${p}`);
    return;
  }
  for (const p of present) fs.rmSync(p, { force: true });
  const left = presentModels(MODELS_DIR);
  if (left.length) {
    console.error(`[release-ocr-models] could not remove: ${left.join(', ')}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = { parseFeatures, enabledFeatures, ocrShipped, presentModels, parseArgs, CARGO_TOML };
