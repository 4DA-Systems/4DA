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

// ── The NuGet source ────────────────────────────────────────────────────────
// The library comes out of one package holding five libraries under two file
// names, so the member is read by its full path, in Node, the same way on
// every platform.

const zlib = require('node:zlib');
const { zipMember, PLATFORMS, NUGET } = require('./download-ort.cjs');

/** A zip archive of `entries` ([path, Buffer, deflate?]), as any zip tool writes it. */
function zipOf(entries) {
  const locals = [];
  const central = [];
  let offset = 0;
  for (const [name, data, deflate] of entries) {
    const nameBuf = Buffer.from(name, 'utf8');
    const body = deflate ? zlib.deflateRawSync(data) : data;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(deflate ? 8 : 0, 8);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    const dir = Buffer.alloc(46);
    dir.writeUInt32LE(0x02014b50, 0);
    dir.writeUInt16LE(deflate ? 8 : 0, 10);
    dir.writeUInt32LE(body.length, 20);
    dir.writeUInt32LE(data.length, 24);
    dir.writeUInt16LE(nameBuf.length, 28);
    dir.writeUInt32LE(offset, 42);
    locals.push(local, nameBuf, body);
    central.push(dir, nameBuf);
    offset += 30 + nameBuf.length + body.length;
  }
  const centralBuf = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralBuf.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, centralBuf, end]);
}

test('a zip member is read by its full path, stored or deflated', () => {
  const x64 = Buffer.from('the x64 library '.repeat(400));
  const arm = Buffer.from('the arm64 library '.repeat(400));
  const zip = zipOf([
    ['runtimes/linux-arm64/native/libonnxruntime.so', arm, true],
    ['runtimes/linux-x64/native/libonnxruntime.so', x64, true],
    ['readme.txt', Buffer.from('stored'), false],
  ]);
  // Two members share a file name: the path decides, not the name.
  assert.deepEqual(zipMember(zip, 'runtimes/linux-x64/native/libonnxruntime.so'), x64);
  assert.deepEqual(zipMember(zip, 'runtimes/linux-arm64/native/libonnxruntime.so'), arm);
  assert.deepEqual(zipMember(zip, 'readme.txt'), Buffer.from('stored'));
  assert.equal(zipMember(zip, 'runtimes/osx-arm64/native/libonnxruntime.dylib'), null);
  assert.throws(() => zipMember(Buffer.from('not a zip at all, just some text'), 'x'), /not a zip archive/);
});

test('every platform that bundles a library pins the package member and the library itself', () => {
  assert.match(NUGET.sha256, /^[0-9a-f]{64}$/);
  assert.match(NUGET.url, /^https:\/\/api\.nuget\.org\/v3-flatcontainer\/microsoft\.ml\.onnxruntime\//);
  const rid = { 'win32-x64': 'win-x64', 'win32-arm64': 'win-arm64', 'darwin-arm64': 'osx-arm64', 'linux-x64': 'linux-x64', 'linux-arm64': 'linux-arm64' };
  const bundled = Object.entries(PLATFORMS).filter(([, spec]) => !spec.bundleNothing);
  assert.deepEqual(bundled.map(([key]) => key).sort(), Object.keys(rid).sort());
  for (const [key, spec] of bundled) {
    // The member is the platform's own: a library for another CPU or OS is never named.
    assert.equal(spec.member, `runtimes/${rid[key]}/native/${spec.lib}`, key);
    assert.match(spec.libSha256, /^[0-9a-f]{64}$/, key);
    assert.match(spec.sha256, /^[0-9a-f]{64}$/, key);
  }
  assert.equal(new Set(bundled.map(([, spec]) => spec.libSha256)).size, bundled.length, 'each library has its own pin');
});
