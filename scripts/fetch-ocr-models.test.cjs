// Tests for scripts/fetch-ocr-models.cjs — the SHA-256 pin on the two OCR model
// blobs that .github/workflows/release.yml bundles into the signed, notarized
// installer.
//
// The defect: those blobs were downloaded from a third-party S3 bucket with NO
// integrity check, and an existing file on disk was trusted purely for
// existing. Anything that landed at that path — stale, truncated, substituted —
// went into an artifact carrying 4DA's EV signature.
//
// These tests do not hit the network. They exercise the verification predicate,
// which is the part that has to be able to say NO, and the mirror -> upstream
// fallback through an injected fetcher.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');

const {
  MODELS,
  LEGACY_FILES,
  MIRROR_BASE,
  UPSTREAM_BASE,
  sha256File,
  verify,
  fetchModel,
  main,
} = require('./fetch-ocr-models.cjs');

function tmpFile(contents) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocr-pin-'));
  const file = path.join(dir, 'blob.onnx');
  fs.writeFileSync(file, contents);
  return file;
}

test('every model carries a pinned digest and byte length', () => {
  assert.equal(MODELS.length, 2);
  for (const m of MODELS) {
    assert.match(m.sha256, /^[0-9a-f]{64}$/, `${m.file} needs a real 64-hex SHA-256, not a placeholder`);
    assert.ok(Number.isInteger(m.bytes) && m.bytes > 0, `${m.file} needs a pinned byte length`);
    for (const url of m.urls) assert.match(url, /^https:\/\//, `${m.file} must be fetched over TLS`);
  }
  // Guard against a copy-paste that pins both files to the same digest.
  assert.notEqual(MODELS[0].sha256, MODELS[1].sha256);
});

test('models are the ONNX exports, and the Rust extractor loads exactly these names', () => {
  // The `.rten` URLs are in an upstream brownout (HTTP 403 since 2026-10-27,
  // robertknight/ocrs#282). rten's Model::load_file picks the format from the
  // file extension, so the name on disk must stay `.onnx`, must match the URL
  // it came from, and must be the name src-tauri/src/extractors/image.rs opens.
  const imageRs = fs.readFileSync(
    path.join(__dirname, '..', 'src-tauri', 'src', 'extractors', 'image.rs'),
    'utf8',
  );
  for (const m of MODELS) {
    assert.match(m.file, /\.onnx$/, `${m.file} must be an ONNX model`);
    for (const url of m.urls) {
      assert.ok(url.endsWith(`/${m.file}`), `${m.file} must be saved under the name it is fetched as`);
    }
    assert.ok(imageRs.includes(`"${m.file}"`), `image.rs must load ${m.file}`);
  }
  assert.doesNotMatch(imageRs, /"text-(detection|recognition)\.rten"/);
});

test('the 4DA mirror is tried first and upstream is the fallback', () => {
  // Upstream asked consumers to host their own copies (robertknight/ocrs#282).
  // The mirror tag must not match release.yml's `v*` trigger.
  assert.match(MIRROR_BASE, /^https:\/\/github\.com\/4DA-Systems\/4DA\/releases\/download\/[^v/][^/]*$/);
  for (const m of MODELS) {
    assert.deepEqual(m.urls, [`${MIRROR_BASE}/${m.file}`, `${UPSTREAM_BASE}/${m.file}`]);
  }
});

test('verify() accepts bytes that match the pin', () => {
  const body = Buffer.from('pretend model bytes');
  const file = tmpFile(body);
  const model = {
    file: 'blob.onnx',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  assert.deepEqual(verify(file, model), { ok: true });
});

test('verify() REJECTS a single flipped byte', () => {
  const body = Buffer.from('pretend model bytes');
  const model = {
    file: 'blob.onnx',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  const tampered = Buffer.from(body);
  tampered[0] ^= 0x01;
  const result = verify(tmpFile(tampered), model);
  assert.equal(result.ok, false);
  assert.match(result.reason, /SHA-256/);
});

test('verify() REJECTS a truncated download before it hashes', () => {
  const body = Buffer.from('pretend model bytes');
  const model = {
    file: 'blob.onnx',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  const result = verify(tmpFile(body.subarray(0, 5)), model);
  assert.equal(result.ok, false);
  assert.match(result.reason, /size 5 != pinned/);
});

test('verify() REJECTS a substituted file of the same length', () => {
  const body = Buffer.from('pretend model bytes');
  const evil = Buffer.from('PRETEND MODEL BYTES');
  assert.equal(body.length, evil.length);
  const model = {
    file: 'blob.onnx',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  assert.equal(verify(tmpFile(evil), model).ok, false);
});

test('verify() REJECTS a missing file rather than passing vacuously', () => {
  const result = verify(path.join(os.tmpdir(), 'definitely-not-here.onnx'), MODELS[0]);
  assert.equal(result.ok, false);
  assert.match(result.reason, /cannot stat/);
});

test('sha256File matches node crypto over the same bytes', () => {
  const body = crypto.randomBytes(4096);
  assert.equal(sha256File(tmpFile(body)), crypto.createHash('sha256').update(body).digest('hex'));
});

// --- fallback: driven through an injected fetcher, no network -------------

const GOOD = Buffer.from('the pinned model bytes');
const EVIL = Buffer.from('THE PINNED MODEL BYTES');
const PRIMARY = 'https://mirror.example/blob.onnx';
const FALLBACK = 'https://upstream.example/blob.onnx';
const fakeModel = () => ({
  file: 'blob.onnx',
  urls: [PRIMARY, FALLBACK],
  bytes: GOOD.length,
  sha256: crypto.createHash('sha256').update(GOOD).digest('hex'),
});

/** A fetcher that serves `routes[url]`: a Buffer, or an Error to throw. Records calls. */
function fakeFetcher(routes) {
  const calls = [];
  const fetcher = async (url, dest) => {
    calls.push(url);
    const r = routes[url];
    if (r instanceof Error) throw r;
    fs.writeFileSync(dest, r);
  };
  return { fetcher, calls };
}

function tmpDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'ocr-fetch-'));
}

test('fetchModel: primary verified -> fallback never contacted', async () => {
  const dest = path.join(tmpDir(), 'blob.onnx');
  const { fetcher, calls } = fakeFetcher({ [PRIMARY]: GOOD, [FALLBACK]: new Error('unused') });
  assert.deepEqual(await fetchModel(fakeModel(), dest, fetcher), { ok: true, url: PRIMARY });
  assert.deepEqual(calls, [PRIMARY]);
  assert.deepEqual(fs.readFileSync(dest), GOOD);
});

test('fetchModel: primary 404 -> falls back to upstream', async () => {
  const dest = path.join(tmpDir(), 'blob.onnx');
  const { fetcher, calls } = fakeFetcher({ [PRIMARY]: new Error(`HTTP 404 for ${PRIMARY}`), [FALLBACK]: GOOD });
  assert.deepEqual(await fetchModel(fakeModel(), dest, fetcher), { ok: true, url: FALLBACK });
  assert.deepEqual(calls, [PRIMARY, FALLBACK]);
  assert.deepEqual(fs.readFileSync(dest), GOOD);
});

test('fetchModel: primary digest mismatch is rejected -> falls back to upstream', async () => {
  const dest = path.join(tmpDir(), 'blob.onnx');
  const { fetcher, calls } = fakeFetcher({ [PRIMARY]: EVIL, [FALLBACK]: GOOD });
  assert.deepEqual(await fetchModel(fakeModel(), dest, fetcher), { ok: true, url: FALLBACK });
  assert.deepEqual(calls, [PRIMARY, FALLBACK]);
  assert.deepEqual(fs.readFileSync(dest), GOOD, 'the mirror bytes must not survive');
});

test('fetchModel: primary mismatch + fallback unreachable -> integrity failure, nothing kept', async () => {
  const dest = path.join(tmpDir(), 'blob.onnx');
  const { fetcher } = fakeFetcher({ [PRIMARY]: EVIL, [FALLBACK]: new Error('HTTP 403') });
  const r = await fetchModel(fakeModel(), dest, fetcher);
  assert.equal(r.ok, false);
  assert.equal(r.integrity, true);
  assert.equal(r.failures.length, 2);
  assert.match(r.failures[0].reason, /INTEGRITY: SHA-256/);
  assert.equal(fs.existsSync(dest), false);
});

test('fetchModel: both unreachable -> network failure (not integrity)', async () => {
  const dest = path.join(tmpDir(), 'blob.onnx');
  const { fetcher } = fakeFetcher({ [PRIMARY]: new Error('HTTP 404'), [FALLBACK]: new Error('HTTP 403') });
  const r = await fetchModel(fakeModel(), dest, fetcher);
  assert.deepEqual(r, {
    ok: false,
    integrity: false,
    failures: [
      { url: PRIMARY, reason: 'HTTP 404' },
      { url: FALLBACK, reason: 'HTTP 403' },
    ],
  });
});

async function runMain(routes, argv = ['node', 'fetch-ocr-models.cjs']) {
  const modelsDir = tmpDir();
  const { fetcher } = fakeFetcher(routes);
  const code = await main(argv, { models: [fakeModel()], modelsDir, fetcher });
  return { code, modelsDir };
}

test('main: both sources serve wrong bytes -> the script errors and keeps nothing', async () => {
  const { code, modelsDir } = await runMain({ [PRIMARY]: EVIL, [FALLBACK]: EVIL });
  assert.equal(code, 1);
  assert.equal(fs.existsSync(path.join(modelsDir, 'blob.onnx')), false);
});

test('main: both sources unreachable with --require -> the script errors', async () => {
  const { code } = await runMain(
    { [PRIMARY]: new Error('HTTP 404'), [FALLBACK]: new Error('HTTP 403') },
    ['node', 'fetch-ocr-models.cjs', '--require'],
  );
  assert.equal(code, 1);
});

test('main: both sources unreachable without --require -> offline install still succeeds', async () => {
  const { code, modelsDir } = await runMain({ [PRIMARY]: new Error('HTTP 404'), [FALLBACK]: new Error('HTTP 403') });
  assert.equal(code, 0);
  assert.equal(fs.existsSync(path.join(modelsDir, 'blob.onnx')), false);
});

test('main: primary 404 -> upstream bytes land on disk, legacy .rten files removed', async () => {
  const modelsDir = tmpDir();
  for (const legacy of LEGACY_FILES) fs.writeFileSync(path.join(modelsDir, legacy), 'old');
  const { fetcher } = fakeFetcher({ [PRIMARY]: new Error('HTTP 404'), [FALLBACK]: GOOD });
  const code = await main(['node', 'x'], { models: [fakeModel()], modelsDir, fetcher });
  assert.equal(code, 0);
  assert.deepEqual(fs.readFileSync(path.join(modelsDir, 'blob.onnx')), GOOD);
  for (const legacy of LEGACY_FILES) assert.equal(fs.existsSync(path.join(modelsDir, legacy)), false);
});

test('the pinned models on disk (if present) match the pin', () => {
  // On a machine that has already run postinstall this is a real end-to-end
  // check of the shipped digests. Where the files are absent — a fresh CI
  // checkout — there is nothing to assert and the pin is covered by the
  // format/uniqueness assertions above.
  for (const m of MODELS) {
    const p = path.join(__dirname, '..', 'src-tauri', 'models', m.file);
    if (!fs.existsSync(p)) continue;
    assert.deepEqual(verify(p, m), { ok: true }, `${m.file} on disk does not match its pin`);
  }
});
