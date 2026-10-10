// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for scripts/release-ocr-models.cjs — the release-time decision that
// keeps the OCR `.rten` weights out of installers built without `ocr`.
//
// Run: node --test scripts/release-ocr-models.test.cjs   (or `pnpm run test:scripts`)

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  parseFeatures,
  enabledFeatures,
  ocrShipped,
  presentModels,
  parseArgs,
  CARGO_TOML,
} = require('./release-ocr-models.cjs');

const TOML = `
[package]
name = "fourda"

[dependencies]
ocrs = { version = "0.12", optional = true }

[features]
default = ["fastembed-local"]
fastembed-local = ["dep:fastembed", "dep:tar"] # trailing comment
ocr = ["dep:ocrs", "dep:rten"]
everything = ["ocr", "archive"]
archive = ["dep:tar"]
uses-dep-feature = ["fastembed/ort"]

[[example]]
name = "test_ocr_full"
`;

test('parses the [features] table and nothing outside it', () => {
  const f = parseFeatures(TOML);
  assert.deepEqual(f.default, ['fastembed-local']);
  assert.deepEqual(f.ocr, ['dep:ocrs', 'dep:rten']);
  assert.deepEqual(f.everything, ['ocr', 'archive']);
  assert.equal(f.name, undefined);
});

test('the default build does not compile OCR', () => {
  assert.equal(ocrShipped(TOML), false);
});

test('asking for ocr, directly or through another feature, ships it', () => {
  assert.equal(ocrShipped(TOML, ['ocr']), true);
  assert.equal(ocrShipped(TOML, ['everything']), true);
});

test('a default that pulls ocr in transitively ships it', () => {
  const toml = TOML.replace('default = ["fastembed-local"]', 'default = ["everything"]');
  assert.equal(ocrShipped(toml), true);
  assert.equal(ocrShipped(toml, [], { noDefault: true }), false);
});

test('dep: and crate/feature entries are never followed as local features', () => {
  const on = enabledFeatures(parseFeatures(TOML), ['uses-dep-feature']);
  assert.deepEqual([...on].sort(), ['default', 'fastembed-local', 'uses-dep-feature']);
});

test('THE CONFIG CHECK: the real Cargo.toml release feature set excludes the OCR models', () => {
  // release.yml builds with `--target` only, so the release feature set is the
  // default set. If this ever fails, OCR became part of the release and the
  // script will fetch + verify the models instead of removing them.
  assert.equal(ocrShipped(fs.readFileSync(CARGO_TOML, 'utf8')), false);
});

test('presentModels lists only the pinned OCR files that exist', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocr-models-'));
  try {
    assert.deepEqual(presentModels(dir), []);
    fs.writeFileSync(path.join(dir, 'text-detection.rten'), 'x');
    fs.mkdirSync(path.join(dir, 'embeddings'));
    assert.deepEqual(presentModels(dir), [path.join(dir, 'text-detection.rten')]);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('parses --features and --dry-run', () => {
  assert.deepEqual(parseArgs(['--features', 'ocr, archive', '--dry-run']), {
    features: ['ocr', 'archive'],
    dryRun: true,
  });
  assert.deepEqual(parseArgs([]), { features: [], dryRun: false });
});
