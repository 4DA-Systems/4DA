// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for scripts/release-embedding-model.cjs — the release-time step that
// keeps the embedding model out of installers (AD-054 decision 7), and the
// config checks that prove nothing else puts it back.
//
// Run: node --test scripts/release-embedding-model.test.cjs   (or `pnpm run test:scripts`)

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { listFiles, isEmbeddingFile, planStrip, formatReport, parseArgs } = require('./release-embedding-model.cjs');

const ROOT = path.resolve(__dirname, '..');

/** A models dir shaped like a release runner's after `pnpm install` + bundle:resources. */
function fakeModels() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'release-embed-'));
  const put = (rel, bytes) => {
    const p = path.join(dir, ...rel.split('/'));
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, Buffer.alloc(bytes));
  };
  put('ort/onnxruntime.dll', 1400);
  put('embeddings/nomic-embed-text-v1.5-fp16/model_fp16.onnx', 27000);
  put('embeddings/nomic-embed-text-v1.5-fp16/tokenizer.json', 70);
  put('embeddings/models--Snowflake--snowflake-arctic-embed-m/x.onnx', 10);
  put('text-detection.rten', 250);
  return dir;
}

test('only paths under embeddings/ are the model', () => {
  assert.equal(isEmbeddingFile('embeddings/nomic-embed-text-v1.5-fp16/model_fp16.onnx'), true);
  assert.equal(isEmbeddingFile('embeddings'), true);
  assert.equal(isEmbeddingFile('ort/onnxruntime.dll'), false);
  assert.equal(isEmbeddingFile('embeddings-notes.txt'), false);
  assert.equal(isEmbeddingFile('text-detection.rten'), false);
});

test('the plan strips every embedding file and keeps the rest', () => {
  const dir = fakeModels();
  try {
    const plan = planStrip(dir);
    assert.deepEqual(
      plan.strip.map((f) => f.rel),
      [
        'embeddings/models--Snowflake--snowflake-arctic-embed-m/x.onnx',
        'embeddings/nomic-embed-text-v1.5-fp16/model_fp16.onnx',
        'embeddings/nomic-embed-text-v1.5-fp16/tokenizer.json',
      ],
    );
    assert.deepEqual(plan.keep.map((f) => f.rel), ['ort/onnxruntime.dll', 'text-detection.rten']);
    assert.equal(plan.stripBytes, 27080);
    assert.equal(plan.keepBytes, 1650);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('the dry-run report names each file and the before/after totals', () => {
  const dir = fakeModels();
  try {
    const report = formatReport(planStrip(dir));
    assert.match(report, /ship .* ort\/onnxruntime\.dll/);
    assert.match(report, /strip .* embeddings\/nomic-embed-text-v1\.5-fp16\/model_fp16\.onnx/);
    assert.match(report, /before 0\.0 MB, after 0\.0 MB/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('the CLI strips the model end to end, and --dry-run touches nothing', () => {
  const dir = fakeModels();
  const { spawnSync } = require('node:child_process');
  const run = (...extra) =>
    spawnSync(process.execPath, [path.join(__dirname, 'release-embedding-model.cjs'), '--models-dir', dir, ...extra], {
      encoding: 'utf8',
    });
  try {
    const dry = run('--dry-run');
    assert.equal(dry.status, 0, dry.stderr);
    assert.match(dry.stdout, /strip .*model_fp16\.onnx/);
    assert.equal(planStrip(dir).strip.length, 3, 'dry run removes nothing');

    const real = run();
    assert.equal(real.status, 0, real.stderr);
    assert.match(real.stdout, /removed 3 file\(s\)/);
    assert.equal(planStrip(dir).strip.length, 0);
    assert.deepEqual(listFiles(dir).map((f) => f.rel), ['ort/onnxruntime.dll', 'text-detection.rten']);

    // Idempotent: a tree without the model passes again.
    assert.equal(run().status, 0);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('parses --dry-run, --bundle and --models-dir', () => {
  const a = parseArgs(['--dry-run', '--models-dir', 'x']);
  assert.equal(a.dryRun, true);
  assert.equal(a.bundle, false);
  assert.equal(a.modelsDir, path.resolve('x'));
  assert.equal(parseArgs(['--bundle']).bundle, true);
  assert.equal(parseArgs([]).modelsDir, path.join(ROOT, 'src-tauri', 'models'));
});

// ---------------------------------------------------------------------------
// THE CONFIG CHECKS: nothing in the release path puts the model back.
// ---------------------------------------------------------------------------

test('bundle:resources (tauri beforeBuildCommand) no longer fetches the model', () => {
  const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
  assert.doesNotMatch(pkg.scripts['bundle:resources'], /download-embedding-model/);
  const conf = JSON.parse(fs.readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));
  assert.match(conf.build.beforeBuildCommand, /bundle:resources/);
  assert.doesNotMatch(conf.build.beforeBuildCommand, /download-embedding-model|bundle:model/);
});

test('release.yml strips the model before the tauri build, without --bundle', () => {
  const wf = fs.readFileSync(path.join(ROOT, '.github', 'workflows', 'release.yml'), 'utf8');
  const strip = wf.indexOf('node scripts/release-embedding-model.cjs');
  const build = wf.indexOf('uses: tauri-apps/tauri-action');
  assert.ok(strip > 0, 'release.yml runs release-embedding-model.cjs');
  assert.ok(build > strip, 'and runs it before the tauri build');
  const line = wf.slice(strip, wf.indexOf('\n', strip));
  assert.doesNotMatch(line, /--bundle/);
  // Nothing between the strip and the build fetches the model again.
  assert.doesNotMatch(wf.slice(strip, build), /download-embedding-model|bundle:model/);
});
