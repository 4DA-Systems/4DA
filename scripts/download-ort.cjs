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
const DEST_DIR = path.resolve(__dirname, '..', 'src-tauri', 'models', 'ort');

const PLATFORMS = {
  'win32-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-x64-${ORT_VERSION}.zip`,
    lib: 'onnxruntime.dll',
    archive: 'zip',
  },
  'win32-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-win-arm64-${ORT_VERSION}.zip`,
    lib: 'onnxruntime.dll',
    archive: 'zip',
  },
  'darwin-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-arm64-${ORT_VERSION}.tgz`,
    lib: 'libonnxruntime.dylib',
    archive: 'tgz',
  },
  'darwin-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-osx-x86_64-${ORT_VERSION}.tgz`,
    lib: 'libonnxruntime.dylib',
    archive: 'tgz',
  },
  'linux-x64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-${ORT_VERSION}.tgz`,
    lib: 'libonnxruntime.so',
    archive: 'tgz',
  },
  'linux-arm64': {
    url: `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-aarch64-${ORT_VERSION}.tgz`,
    lib: 'libonnxruntime.so',
    archive: 'tgz',
  },
};

function detectPlatform() {
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
          if (total > 0) {
            const pct = ((downloaded / total) * 100).toFixed(1);
            const mb = (downloaded / 1048576).toFixed(1);
            const totalMb = (total / 1048576).toFixed(1);
            process.stdout.write(`\r  ${mb}MB / ${totalMb}MB (${pct}%)`);
          }
        });
        res.on('end', () => {
          if (total > 0) process.stdout.write('\n');
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

function extractTgzLib(buffer, libName, destPath) {
  const tmpTgz = path.join(DEST_DIR, '_tmp.tgz');
  fs.writeFileSync(tmpTgz, buffer);
  try {
    execSync(
      `tar xzf "${tmpTgz}" --strip-components=2 -C "${DEST_DIR}" --include="*/${libName}"`,
      { stdio: 'pipe' }
    );
  } finally {
    try { fs.unlinkSync(tmpTgz); } catch {}
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
  if (fs.existsSync(destPath)) {
    const size = fs.statSync(destPath).size;
    if (size > 1_000_000) {
      console.log(`  ${platformKey}: ${spec.lib} already exists (${(size / 1048576).toFixed(1)}MB) — skipping`);
      return;
    }
  }

  console.log(`  ${platformKey}: downloading ORT ${ORT_VERSION}...`);
  const buffer = await download(spec.url);
  console.log(`  ${platformKey}: extracting ${spec.lib}...`);

  if (spec.archive === 'zip') {
    extractZipLib(buffer, spec.lib, destPath);
  } else {
    extractTgzLib(buffer, spec.lib, destPath);
  }

  if (!fs.existsSync(destPath)) {
    console.error(`  FAILED: ${spec.lib} not found after extraction`);
    process.exit(1);
  }

  const finalSize = (fs.statSync(destPath).size / 1048576).toFixed(1);
  console.log(`  ${platformKey}: ${spec.lib} ready (${finalSize}MB)`);
}

async function main() {
  fs.mkdirSync(DEST_DIR, { recursive: true });

  const args = process.argv.slice(2);
  const platformArg = args.find(a => a.startsWith('--platform='));
  const all = args.includes('--all');

  console.log(`ORT Bundler — ONNX Runtime v${ORT_VERSION}`);
  console.log(`Destination: ${DEST_DIR}\n`);

  if (all) {
    for (const key of Object.keys(PLATFORMS)) {
      await downloadForPlatform(key);
    }
  } else {
    const platform = platformArg ? platformArg.split('=')[1] : detectPlatform();
    await downloadForPlatform(platform);
  }

  console.log('\nDone. ORT libraries ready for Tauri bundling.');
}

main().catch((err) => {
  console.error(`\nFATAL: ${err.message}`);
  process.exit(1);
});
