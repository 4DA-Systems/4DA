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
const zlib = require('zlib');
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

// Every download is pinned by SHA-256 (audit 2026-11-11, P4): the library is
// loaded into the app's process, and the archive used to be trusted on "exists
// and is over 1 MB".
//
// Two sources, tried in order, each pinned on its own:
//
//   1. Microsoft's NuGet package, Microsoft.ML.OnnxRuntime (NUGET below). One
//      package holds every platform's library under runtimes/<rid>/native/.
//      The package is pinned by SHA-256 (the same bytes carry the SHA-512
//      packageHash in nuget.org's own catalog entry for 1.24.2), and each
//      library taken out of it is pinned by SHA-256 as well (`libSha256`).
//   2. The GitHub release archive (`url`, `sha256`). The digests are GitHub's
//      own for the v1.24.2 release assets, checked 2026-11-11.
//
// NuGet is first because the GitHub repository microsoft/onnxruntime, with its
// releases, started returning 404 around 2026-11-17 and still did on
// 2026-11-23: every build that bundles the library failed at this script. The
// libraries are the same files: the win-x64 onnxruntime.dll in the package is
// byte-identical (SHA-256 114947d6...) to the one extracted from the pinned
// GitHub zip before the repository went away. A version bump must update every
// pin here and the five in src-tauri/src/embeddings_providers/ort_runtime.rs.
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

const NUGET = {
  url: `https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/${ORT_VERSION}/microsoft.ml.onnxruntime.${ORT_VERSION}.nupkg`,
  sha256: '782564d3d68e87269ea1812cc94710ad06fe014faf093f01a6f89d0bd58719d3',
};

const PLATFORMS = {
  'win32-x64': {
    member: 'runtimes/win-x64/native/onnxruntime.dll',
    libSha256: '114947d633e6844ce3c4b51ef6678f776628571d08a5763859c61642c8dcca9c',
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-x64-${ORT_VERSION}.zip`,
    sha256: '8e3e9c826375352e29cb2614fe44f3d7a4b0ff7b8028ad7a456af9d949a7e8b0',
    lib: 'onnxruntime.dll',
    archive: 'zip',
    arch: 'x64',
  },
  'win32-arm64': {
    member: 'runtimes/win-arm64/native/onnxruntime.dll',
    libSha256: 'd4c4d939c8bd1e93a86bc5a45b37a1fdd08dce34d8231db014a7e4a023923f5a',
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-arm64-${ORT_VERSION}.zip`,
    sha256: 'dd8180d98e5a0ead7ead99029acc80b86a8b905b9aba4cc978e388039bb5823b',
    lib: 'onnxruntime.dll',
    archive: 'zip',
    arch: 'arm64',
  },
  'darwin-arm64': {
    member: 'runtimes/osx-arm64/native/libonnxruntime.dylib',
    libSha256: '8e88284d3b4395b1e532831ac05eb0ffa3673a62df32349e1b0fd64542cb827d',
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
    member: 'runtimes/linux-x64/native/libonnxruntime.so',
    libSha256: 'ffc84d48e845cf0b562ba4ea5ca32aaafc0d4069019fef4f63095b307d0270ad',
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-${ORT_VERSION}.tgz`,
    sha256: '43725474ba5663642e17684717946693850e2005efbd724ac72da278fead25e6',
    lib: 'libonnxruntime.so',
    archive: 'tgz',
    arch: 'x64',
  },
  'linux-arm64': {
    member: 'runtimes/linux-arm64/native/libonnxruntime.so',
    libSha256: 'e18fe095919d8613ead3a31ff78212bde4fad929418a9b48f49d61c829ed5c82',
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

function sha256Hex(buffer) {
  return require('crypto').createHash('sha256').update(buffer).digest('hex');
}

/**
 * One member of a zip archive, by its full path, or null when the archive has
 * no such member. Read here rather than by `unzip` or PowerShell so that every
 * platform takes the same path, and by full path because the NuGet package
 * holds five libraries under two file names.
 */
function zipMember(buffer, memberPath) {
  let eocd = -1;
  for (let i = buffer.length - 22, min = Math.max(0, buffer.length - 65557); i >= min; i--) {
    if (buffer.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) throw new Error('not a zip archive: no end-of-central-directory record');
  const count = buffer.readUInt16LE(eocd + 10);
  if (count === 0xffff) throw new Error('zip64 archives are not supported');
  let p = buffer.readUInt32LE(eocd + 16);
  for (let n = 0; n < count; n++) {
    if (p + 46 > buffer.length || buffer.readUInt32LE(p) !== 0x02014b50) throw new Error('corrupt zip central directory');
    const method = buffer.readUInt16LE(p + 10);
    const packed = buffer.readUInt32LE(p + 20);
    const size = buffer.readUInt32LE(p + 24);
    const nameLen = buffer.readUInt16LE(p + 28);
    const local = buffer.readUInt32LE(p + 42);
    if (buffer.toString('utf8', p + 46, p + 46 + nameLen) === memberPath) {
      if (local + 30 > buffer.length || buffer.readUInt32LE(local) !== 0x04034b50) throw new Error(`corrupt zip entry for ${memberPath}`);
      const start = local + 30 + buffer.readUInt16LE(local + 26) + buffer.readUInt16LE(local + 28);
      const data = buffer.subarray(start, start + packed);
      let out;
      if (method === 0) out = Buffer.from(data);
      else if (method === 8) out = zlib.inflateRawSync(data);
      else throw new Error(`${memberPath} uses zip compression method ${method}, which is not supported`);
      if (out.length !== size) throw new Error(`${memberPath} is ${out.length} bytes after extraction, the archive says ${size}`);
      return out;
    }
    p += 46 + nameLen + buffer.readUInt16LE(p + 30) + buffer.readUInt16LE(p + 32);
  }
  return null;
}

async function downloadWithRetry(label, url) {
  // Retry transient network failures (e.g. ECONNRESET mid-download) so a single
  // blip doesn't fail the whole signed release.
  for (let attempt = 1; ; attempt++) {
    try {
      return await download(url);
    } catch (e) {
      if (attempt >= 4) throw e;
      console.error(`\n  ${label}: download attempt ${attempt}/4 failed (${e.message}); retrying...`);
      await new Promise((r) => setTimeout(r, 2000 * attempt));
    }
  }
}

// The NuGet package is fetched and checked once, however many platforms ask.
let nugetPackage = null;
function fetchNugetPackage() {
  if (!nugetPackage) {
    nugetPackage = downloadWithRetry('NuGet package', NUGET.url).then((buffer) => {
      const digest = sha256Hex(buffer);
      if (digest !== NUGET.sha256) {
        throw new Error(`the NuGet package's SHA-256 is ${digest}, expected ${NUGET.sha256} (${NUGET.url}). Nothing was taken from it.`);
      }
      return buffer;
    });
  }
  return nugetPackage;
}

/** Source 1: the library out of the pinned NuGet package, itself pinned. */
async function fromNuget(platformKey, spec, destPath) {
  const pkg = await fetchNugetPackage();
  const lib = zipMember(pkg, spec.member);
  if (!lib) throw new Error(`${spec.member} is not in the NuGet package`);
  const digest = sha256Hex(lib);
  if (digest !== spec.libSha256) {
    throw new Error(`${spec.member} in the NuGet package has SHA-256 ${digest}, expected ${spec.libSha256}. Not bundled.`);
  }
  fs.writeFileSync(destPath, lib, { mode: 0o755 });
  console.log(`  ${platformKey}: ${spec.lib} taken from the NuGet package; package and library SHA-256 match their pins`);
}

/** Source 2: the library out of the pinned GitHub release archive. */
async function fromGithubArchive(platformKey, spec, destPath) {
  const buffer = await downloadWithRetry(platformKey, spec.url);
  const digest = sha256Hex(buffer);
  if (digest !== spec.sha256) {
    throw new Error(
      `the downloaded archive's SHA-256 is ${digest}, expected ${spec.sha256} ` +
        `(${spec.url}). Not extracted: a library that does not match its pin is never bundled.`,
    );
  }
  console.log(`  ${platformKey}: archive SHA-256 matches the pin; extracting ${spec.lib}...`);
  if (spec.archive === 'zip') {
    extractZipLib(buffer, spec.lib, destPath);
  } else {
    extractTgzLib(buffer, spec.lib, destPath);
  }
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

  console.log(`  ${platformKey}: fetching ORT ${ORT_VERSION}...`);
  const failures = [];
  for (const [name, source] of [['NuGet', fromNuget], ['GitHub release', fromGithubArchive]]) {
    try {
      await source(platformKey, spec, destPath);
      failures.length = 0;
      break;
    } catch (e) {
      try { fs.unlinkSync(destPath); } catch {}
      failures.push(`${name}: ${e.message}`);
      console.error(`  ${platformKey}: the ${name} source failed (${e.message})`);
    }
  }
  if (failures.length) {
    throw new Error(`${platformKey}: no source delivered ${spec.lib}. ${failures.join(' | ')}`);
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

if (require.main === module) {
  main()
    .then(closeIdleConnections)
    .catch((err) => {
      console.error(`\nFATAL: ${err.message}`);
      process.exit(1);
    });
}

module.exports = { zipMember, PLATFORMS, NUGET };
