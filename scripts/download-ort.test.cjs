// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Tests for scripts/download-ort.cjs on the Intel macOS target (audit
// 2026-11-11, R10). The Intel macOS build ships with local embeddings declared
// unavailable, so for x86_64-apple-darwin the script must bundle NOTHING, exit
// 0 (the release leg builds), and remove any library a build for another
// target left behind (v1.0.3's Intel app shipped an ARM64 dylib). No network:
// the darwin-x64 path never downloads.
//
// Run: node --test scripts/download-ort.test.cjs   (or `pnpm run test:scripts`)

const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const SCRIPT = path.join(__dirname, 'download-ort.cjs');

function run(dest, target) {
  return spawnSync(process.execPath, [SCRIPT], {
    env: { ...process.env, FOURDA_ORT_DEST_DIR: dest, FOURDA_ORT_TARGET: target, CI: '1' },
    encoding: 'utf8',
  });
}

/** A Mach-O 64-bit header naming ARM64 (cputype 0x0100000C), padded past the 1 MB "real file" bar. */
function arm64Dylib(file) {
  const b = Buffer.alloc(1_100_000);
  b.writeUInt32LE(0xfeedfacf, 0);
  b.writeUInt32LE(0x0100000c, 4);
  fs.writeFileSync(file, b);
}

test('Intel macOS target: exits 0, bundles nothing, keeps the resource dir', () => {
  const dest = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'ort-x64-')), 'models', 'ort');
  const r = run(dest, 'x86_64-apple-darwin');
  assert.equal(r.status, 0, r.stderr + r.stdout);
  assert.match(r.stdout, /darwin-x64: no ONNX Runtime bundled/);
  assert.ok(fs.existsSync(dest), 'models/ort exists so the tauri `models` resource still resolves');
  assert.deepEqual(fs.readdirSync(dest), []);
});

test('Intel macOS target: a library left by another target is removed, never bundled', () => {
  const dest = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'ort-x64-')), 'models', 'ort');
  fs.mkdirSync(dest, { recursive: true });
  arm64Dylib(path.join(dest, 'libonnxruntime.dylib'));
  const r = run(dest, 'x86_64-apple-darwin');
  assert.equal(r.status, 0, r.stderr + r.stdout);
  assert.match(r.stdout, /removing libonnxruntime\.dylib \(built for arm64\)/);
  assert.equal(fs.existsSync(path.join(dest, 'libonnxruntime.dylib')), false);
});

test('an Apple-silicon library already in place is kept for the Apple-silicon target', () => {
  const dest = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'ort-arm-')), 'models', 'ort');
  fs.mkdirSync(dest, { recursive: true });
  arm64Dylib(path.join(dest, 'libonnxruntime.dylib'));
  const r = run(dest, 'aarch64-apple-darwin');
  assert.equal(r.status, 0, r.stderr + r.stdout);
  assert.match(r.stdout, /already exists .*arm64/);
  assert.ok(fs.existsSync(path.join(dest, 'libonnxruntime.dylib')));
});
