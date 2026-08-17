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
// which is the part that has to be able to say NO.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');

const { MODELS, sha256File, verify } = require('./fetch-ocr-models.cjs');

function tmpFile(contents) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocr-pin-'));
  const file = path.join(dir, 'blob.rten');
  fs.writeFileSync(file, contents);
  return file;
}

test('every model carries a pinned digest and byte length', () => {
  assert.equal(MODELS.length, 2);
  for (const m of MODELS) {
    assert.match(m.sha256, /^[0-9a-f]{64}$/, `${m.file} needs a real 64-hex SHA-256, not a placeholder`);
    assert.ok(Number.isInteger(m.bytes) && m.bytes > 0, `${m.file} needs a pinned byte length`);
    assert.match(m.url, /^https:\/\//, `${m.file} must be fetched over TLS`);
  }
  // Guard against a copy-paste that pins both files to the same digest.
  assert.notEqual(MODELS[0].sha256, MODELS[1].sha256);
});

test('verify() accepts bytes that match the pin', () => {
  const body = Buffer.from('pretend model bytes');
  const file = tmpFile(body);
  const model = {
    file: 'blob.rten',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  assert.deepEqual(verify(file, model), { ok: true });
});

test('verify() REJECTS a single flipped byte', () => {
  const body = Buffer.from('pretend model bytes');
  const model = {
    file: 'blob.rten',
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
    file: 'blob.rten',
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
    file: 'blob.rten',
    bytes: body.length,
    sha256: crypto.createHash('sha256').update(body).digest('hex'),
  };
  assert.equal(verify(tmpFile(evil), model).ok, false);
});

test('verify() REJECTS a missing file rather than passing vacuously', () => {
  const result = verify(path.join(os.tmpdir(), 'definitely-not-here.rten'), MODELS[0]);
  assert.equal(result.ok, false);
  assert.match(result.reason, /cannot stat/);
});

test('sha256File matches node crypto over the same bytes', () => {
  const body = crypto.randomBytes(4096);
  assert.equal(sha256File(tmpFile(body)), crypto.createHash('sha256').update(body).digest('hex'));
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
