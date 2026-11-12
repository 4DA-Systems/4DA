/**
 * Download platform-specific ONNX Runtime for bundling with the Tauri installer.
 *
 * Fetches the correct ORT shared library for the current (or specified) platform
 * and places it in src-tauri/models/ort/ where Tauri's resource bundler picks it up.
 *
 * Usage:
 *   node scripts/download-ort.cjs                    # current platform
 *   node scripts/download-ort.cjs --platform=darwin-arm64
 *   node scripts/download-ort.cjs --platform=win32-x64
 *   node scripts/download-ort.cjs --all              # all 5 platforms
 *
 * Platforms: win32-x64, win32-arm64, darwin-arm64, darwin-x64, linux-x64, linux-arm64
 *
 * Run before `cargo tauri build` so the installer ships with ORT pre-bundled
 * and users skip the first-run download entirely.
 */

'use strict';

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ORT_VERSION = '1.24.2';
// FOURDA_ORT_DEST_DIR points it at another tree (tests).
const DEST_DIR = process.env.FOURDA_ORT_DEST_DIR || path.resolve(__dirname, '..', 'src-tauri', 'models', 'ort');

// A \r-redrawn progress bar is meaningful on a terminal and is pure noise
// anywhere else — on a pipe every redraw becomes its own log line (~500 of them
// for this archive, ~7,300 for the 105 MB model). Cosmetic, but CI logs should
// be readable.
const SHOW_PROGRESS = process.stdout.isTTY === true && !process.env.CI;

// Node pools HTTPS keep-alive sockets, and an idle TLS socket left in the pool
// holds the event loop open long after the last byte has arrived — the process
// just sits there until the socket ages out. Measured here: the 70.6 MB archive
// finished downloading at +5.8s and the process did not exit until +30.1s, with
// one live TLSSocket (destroyed=false) as the only remaining handle. In CI that
// showed up as a dead-flat 29.7s (+/-0.08s) tail on this script and 240.00s
// (+/-0.01s) on download-embedding-model.cjs, six runs running, which reads as
// a slow download and is nothing of the kind. Drop the pooled sockets once the
// work is done and the process exits immediately (verified: 6.6s, not 30.1s).
function closeIdleConnections() {
  https.globalAgent.destroy();
  require('http').globalAgent.destroy();
}

// Every archive is pinned by SHA-256 (audit 2026-11-11, P4): the library is
// loaded into the app's process, and the archive used to be trusted on "exists
// and is over 1 MB". The digests are GitHub's own for the v1.24.2 release
// assets (`gh api repos/microsoft/onnxruntime/releases/tags/v1.24.2`, field
// `digest`), checked 2026-11-11. A version bump must update them.
//
// darwin-x64 has no archive: Microsoft stopped publishing Intel macOS builds
// after 1.23.2, and this app's `ort` crate needs ONNX Runtime API 24 (>= 1.24),
// so no published build loads on an Intel Mac. v1.0.3's Intel app shipped the
// ARM64 library instead (the release built that leg on an Apple-silicon runner
// and this script picked the runner's CPU). Asking for darwin-x64 now fails with
// the reason rather than shipping a library the app cannot load.
//
// Decision (Wave 1, 2026-11-12): the Intel macOS build ships with local
// embeddings declared unavailable, and Ollama serves embeddings there. So
// darwin-x64 is an explicit, documented skip: this script bundles NOTHING for
// it (and removes any library a build for another target left behind), and
// the app reports "Local embeddings are not available on Intel Macs"
// (src-tauri/src/embeddings_providers/ort_runtime.rs, INTEL_MAC_UNAVAILABLE).
// release.yml's dylib pre-sign step skips when no dylib is present. The
// `models` resource still resolves: this script always creates models/ort/.
const DARWIN_X64_SKIP =
  `ONNX Runtime ${ORT_VERSION} publishes no Intel macOS build (the last one was 1.23.2, ` +
  `and this app's ort crate needs ONNX Runtime API 24, i.e. >= 1.24), so the Intel macOS ` +
  `build bundles no ONNX Runtime: local embeddings are declared unavailable there and ` +
  `Ollama serves embeddings instead.`;

const PLATFORMS = {
  'win32-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-x64-${ORT_VERSION}.zip`,
    sha256: '8e3e9c826375352e29cb2614fe44f3d7a4b0ff7b8028ad7a456af9d949a7e8b0',
    lib: 'onnxruntime.dll',
    archive: 'zip',
    arch: 'x64',
  },
  'win32-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-arm64-${ORT_VERSION}.zip`,
    sha256: 'dd8180d98e5a0ead7ead99029acc80b86a8b905b9aba4cc978e388039bb5823b',
    lib: 'onnxruntime.dll',
    archive: 'zip',
    arch: 'arm64',
  },
  'darwin-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-arm64-${ORT_VERSION}.tgz`,
    sha256: '0af4fa503e8ea285245b47ee42d0a7461b8156a81270857da0c1d4ecf858abde',
    lib: 'libonnxruntime.dylib',
    archive: 'tgz',
    arch: 'arm64',
  },
  'darwin-x64': {
    bundleNothing: DARWIN_X64_SKIP,
    lib: 'libonnxruntime.dylib',
    archive: 'tgz',
    arch: 'x64',
  },
  'linux-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-${ORT_VERSION}.tgz`,
    sha256: '43725474ba5663642e17684717946693850e2005efbd724ac72da278fead25e6',
    lib: 'libonnxruntime.so',
    archive: 'tgz',
    arch: 'x64',
  },
  'linux-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-aarch64-${ORT_VERSION}.tgz`,
    sha256: '6715b3d19965a2a6981e78ed4ba24f17a8c30d2d26420dbed10aac7ceca0085e',
    lib: 'libonnxruntime.so',
    archive: 'tgz',
    arch: 'arm64',
  },
};

/** The platform a Rust target triple builds for, or null for one this script does not know. */
function platformForTriple(triple) {
  const t = String(triple || '').toLowerCase();
  const arch = t.startsWith('aarch64') || t.startsWith('arm64') ? 'arm64' : t.startsWith('x86_64') ? 'x64' : null;
  if (!arch) return null;
  if (t.includes('windows')) return `win32-${arch}`;
  if (t.includes('apple-darwin')) return `darwin-${arch}`;
  if (t.includes('linux')) return `linux-${arch}`;
  return null;
}

/**
 * The CPU a native library is built for, read from its header: 'x64', 'arm64',
 * 'universal' (a macOS fat binary), or null when the header is not one we know.
 * PE (Windows): machine field after the "PE\0\0" signature. Mach-O 64-bit
 * (macOS): cputype at offset 4. ELF (Linux): e_machine at offset 18.
 */
function libArch(file) {
  let h;
  try {
    const fd = fs.openSync(file, 'r');
    h = Buffer.alloc(4096);
    fs.readSync(fd, h, 0, 4096, 0);
    fs.closeSync(fd);
  } catch {
    return null;
  }
  if (h[0] === 0x4d && h[1] === 0x5a) {
    const pe = h.readUInt32LE(0x3c);
    if (pe + 6 > h.length || h.readUInt32LE(pe) !== 0x00004550) return null;
    const machine = h.readUInt16LE(pe + 4);
    return machine === 0x8664 ? 'x64' : machine === 0xaa64 ? 'arm64' : null;
  }
  if (h.readUInt32LE(0) === 0xfeedfacf) {
    const cpu = h.readUInt32LE(4);
    return cpu === 0x01000007 ? 'x64' : cpu === 0x0100000c ? 'arm64' : null;
  }
  if (h.readUInt32BE(0) === 0xcafebabe) return 'universal';
  if (h[0] === 0x7f && h[1] === 0x45 && h[2] === 0x4c && h[3] === 0x46) {
    const machine = h.readUInt16LE(18);
    return machine === 0x3e ? 'x64' : machine === 0xb7 ? 'arm64' : null;
  }
  return null;
}

/** Whether a library built for `found` runs on the platform's CPU. */
function archMatches(found, wanted) {
  return found === wanted || found === 'universal';
}

/**
 * The platform to bundle for. A Tauri build passes its TARGET in
 * TAURI_ENV_TARGET_TRIPLE (also FOURDA_ORT_TARGET for a direct call): the
 * runner's own CPU is the wrong answer for a cross-build, which is how v1.0.3's
 * Intel macOS app, built on an Apple-silicon runner, shipped an ARM64 library.
 */
function detectPlatform() {
  for (const name of ['FOURDA_ORT_TARGET', 'TAURI_ENV_TARGET_TRIPLE']) {
    const fromTarget = platformForTriple(process.env[name]);
    if (fromTarget) return fromTarget;
  }
  const os = process.platform;
  const arch = process.arch;
  if (os === 'win32') return arch === 'arm64' ? 'win32-arm64' : 'win32-x64';
  if (os === 'darwin') return arch === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
  return arch === 'arm64' ? 'linux-arm64' : 'linux-x64';
}

function download(url) {
  return new Promise((resolve, reject) => {
    const follow = (u, redirects = 0) => {
      if (redirects > 5) return reject(new Error('Too many redirects'));
      const mod = u.startsWith('https') ? https : require('http');
      mod.get(u, { headers: { 'User-Agent': '4DA-build' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return follow(res.headers.location, redirects + 1);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${u}`));
        }
        const chunks = [];
        let downloaded = 0;
        const total = parseInt(res.headers['content-length'] || '0', 10);
        res.on('data', (chunk) => {
          chunks.push(chunk);
          downloaded += chunk.length;
          if (total > 0 && SHOW_PROGRESS) {
            const pct = ((downloaded / total) * 100).toFixed(1);
            const mb = (downloaded / 1048576).toFixed(1);
            const totalMb = (total / 1048576).toFixed(1);
            process.stdout.write(`\r  ${mb}MB / ${totalMb}MB (${pct}%)`);
          }
        });
        res.on('end', () => {
          if (total > 0 && SHOW_PROGRESS) process.stdout.write('\n');
          resolve(Buffer.concat(chunks));
        });
        res.on('error', reject);
      }).on('error', reject);
    };
    follow(url);
  });
}

function extractZipLib(buffer, libName, destPath) {
  const tmpZip = path.join(DEST_DIR, '_tmp.zip');
  fs.writeFileSync(tmpZip, buffer);
  try {
    if (process.platform === 'win32') {
      execSync(
        `powershell -NoProfile -Command "` +
        // Windows PowerShell 5.1 does not auto-load this assembly — load it
        // explicitly or ZipFile/ZipFileExtensions resolve to TypeNotFound.
        `Add-Type -AssemblyName System.IO.Compression.FileSystem; ` +
        `$zip = [System.IO.Compression.ZipFile]::OpenRead('${tmpZip.replace(/'/g, "''")}'); ` +
        `$entry = $zip.Entries | Where-Object { $_.Name -eq '${libName}' } | Select-Object -First 1; ` +
        `[System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, '${destPath.replace(/'/g, "''")}', $true); ` +
        `$zip.Dispose()"`,
        { stdio: 'pipe' }
      );
    } else {
      execSync(`unzip -o -j "${tmpZip}" "*/${libName}" -d "${DEST_DIR}"`, { stdio: 'pipe' });
    }
  } finally {
    try { fs.unlinkSync(tmpZip); } catch {}
  }
}

// Recursively find a file by exact name under `dir`. Returns the full path or null.
function findFileByName(dir, name) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const hit = findFileByName(full, name);
      if (hit) return hit;
    } else if (entry.name === name) {
      return full;
    }
  }
  return null;
}

function extractTgzLib(buffer, libName, destPath) {
  const tmpDir = path.join(DEST_DIR, '_tmp_extract');
  try {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    fs.mkdirSync(tmpDir, { recursive: true });
    // Write the archive INTO the extract dir and invoke tar with cwd + a
    // RELATIVE name. Two cross-platform traps this avoids:
    //   1. The previous `--include="*/lib"` is a bsdtar/libarchive flag that GNU
    //      tar (Linux) does NOT understand -> the Linux release leg died with a
    //      fatal tar error. Plain `tar xzf` works on GNU tar + bsdtar alike.
    //   2. An absolute Windows path (`D:\...tgz`) makes GNU tar treat `D:` as a
    //      remote host ("Cannot connect to D:"). A relative name under cwd
    //      sidesteps that on every tar flavor.
    // Extract everything, then locate the lib in Node (tar-flavor-agnostic).
    const tgzName = '_ort.tgz';
    fs.writeFileSync(path.join(tmpDir, tgzName), buffer);
    execSync(`tar xzf "${tgzName}"`, { cwd: tmpDir, stdio: 'pipe' });
    const found = findFileByName(tmpDir, libName);
    if (!found) {
      throw new Error(`${libName} not found inside the downloaded ORT archive`);
    }
    fs.copyFileSync(found, destPath);
  } finally {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch {}
  }
}

async function downloadForPlatform(platformKey) {
  const spec = PLATFORMS[platformKey];
  if (!spec) {
    console.error(`Unknown platform: ${platformKey}`);
    console.error(`Available: ${Object.keys(PLATFORMS).join(', ')}`);
    process.exit(1);
  }

  const destPath = path.join(DEST_DIR, spec.lib);
  if (spec.bundleNothing) {
    // Nothing ships for this platform, so ANY library here came from a build
    // for another target (v1.0.3's Intel app shipped exactly that).
    if (fs.existsSync(destPath)) {
      console.log(`  ${platformKey}: removing ${spec.lib} (built for ${libArch(destPath) ?? 'an unknown CPU'}); nothing is bundled for ${platformKey}`);
      fs.unlinkSync(destPath);
    }
    console.log(`  ${platformKey}: no ONNX Runtime bundled. ${spec.bundleNothing}`);
    return;
  }
  if (fs.existsSync(destPath)) {
    const size = fs.statSync(destPath).size;
    const found = libArch(destPath);
    if (size > 1_000_000 && archMatches(found, spec.arch)) {
      console.log(`  ${platformKey}: ${spec.lib} already exists (${(size / 1048576).toFixed(1)}MB, ${found}) — skipping`);
      return;
    }
    // A library for another CPU (left by a build for another target) is never bundled.
    console.log(`  ${platformKey}: removing ${spec.lib} built for ${found ?? 'an unknown CPU'}; ${platformKey} needs ${spec.arch}`);
    fs.unlinkSync(destPath);
  }

  console.log(`  ${platformKey}: downloading ORT ${ORT_VERSION}...`);
  // Retry transient network failures (e.g. ECONNRESET mid-download) so a single
  // blip doesn't fail the whole signed release.
  let buffer;
  for (let attempt = 1; ; attempt++) {
    try {
      buffer = await download(spec.url);
      break;
    } catch (e) {
      if (attempt >= 4) throw e;
      console.error(`\n  ${platformKey}: download attempt ${attempt}/4 failed (${e.message}); retrying...`);
      await new Promise((r) => setTimeout(r, 2000 * attempt));
    }
  }
  const digest = require('crypto').createHash('sha256').update(buffer).digest('hex');
  if (digest !== spec.sha256) {
    throw new Error(
      `${platformKey}: the downloaded archive's SHA-256 is ${digest}, expected ${spec.sha256} ` +
        `(${spec.url}). Not extracted: a library that does not match its pin is never bundled.`,
    );
  }
  console.log(`  ${platformKey}: SHA-256 matches the pin; extracting ${spec.lib}...`);

  if (spec.archive === 'zip') {
    extractZipLib(buffer, spec.lib, destPath);
  } else {
    extractTgzLib(buffer, spec.lib, destPath);
  }

  if (!fs.existsSync(destPath)) {
    console.error(`  FAILED: ${spec.lib} not found after extraction`);
    process.exit(1);
  }
  const extractedArch = libArch(destPath);
  if (!archMatches(extractedArch, spec.arch)) {
    fs.unlinkSync(destPath);
    throw new Error(`${platformKey}: the extracted ${spec.lib} is built for ${extractedArch ?? 'an unknown CPU'}, not ${spec.arch}; removed`);
  }

  const finalSize = (fs.statSync(destPath).size / 1048576).toFixed(1);
  console.log(`  ${platformKey}: ${spec.lib} ready (${finalSize}MB)`);

  // NOTE: the macOS dylib is code-signed for notarization by a dedicated
  // release.yml step (which imports the Apple cert into a keychain first) - NOT
  // here: bundle:resources runs as tauri's beforeBuildCommand before any signing
  // keychain exists ("item could not be found in the keychain").
}

async function main() {
  fs.mkdirSync(DEST_DIR, { recursive: true });

  const args = process.argv.slice(2);
  const platformArg = args.find(a => a.startsWith('--platform='));
  const all = args.includes('--all');

  console.log(`ORT Bundler — ONNX Runtime v${ORT_VERSION}`);
  console.log(`Destination: ${DEST_DIR}\n`);

  if (all) {
    for (const [key, spec] of Object.entries(PLATFORMS)) {
      if (spec.bundleNothing) {
        console.warn(`  ${key}: skipped. ${spec.bundleNothing}`);
        continue;
      }
      await downloadForPlatform(key);
    }
  } else {
    const platform = platformArg ? platformArg.split('=')[1] : detectPlatform();
    await downloadForPlatform(platform);
  }

  console.log('\nDone. ORT libraries ready for Tauri bundling.');
}

main()
  .then(closeIdleConnections)
  .catch((err) => {
    console.error(`\nFATAL: ${err.message}`);
    process.exit(1);
  });
