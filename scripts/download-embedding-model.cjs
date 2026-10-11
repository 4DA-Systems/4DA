/**
 * Download the in-process embedding model into src-tauri/models/embeddings.
 *
 * Fetches nomic-embed-text v1.5 (fp16 ONNX weights, f32 inputs/outputs, 768-dim)
 * from HuggingFace at a PINNED revision and verifies every file's SHA-256. It is
 * the model Ollama serves by default, so the in-process route writes the same
 * vector space as the Ollama route (measured median cosine 1.00000 vs Ollama's
 * nomic).
 *
 * Release installers no longer carry it (AD-054 decision 7): the app downloads
 * the same pinned files on demand (src-tauri/src/embeddings_providers/model_store.rs,
 * whose FILES must match this list), and scripts/release-embedding-model.cjs
 * strips any copy from the release bundle. This script is for a dev tree, the
 * CI real-embedding gate, and `release-embedding-model.cjs --bundle`. In a debug
 * build the app reads src-tauri/models as its bundled dir, so a fetched copy is
 * used in place.
 *
 * Usage:
 *   node scripts/download-embedding-model.cjs          # download model
 *   node scripts/download-embedding-model.cjs --force   # re-download even if cached
 */

'use strict';

const https = require('https');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const MODEL_REPO = 'nomic-ai/nomic-embed-text-v1.5';
// Pinned: a bump is a reviewed change (it changes every user's vector space).
const REVISION = 'e9b6763023c676ca8431644204f50c2b100d9aab';
const MODEL_DIR_NAME = 'nomic-embed-text-v1.5-fp16';
// Retired bundles removed from the resources dir so they are never shipped again.
const LEGACY_DIR_NAMES = ['models--Snowflake--snowflake-arctic-embed-m'];

const FILES = [
  {
    remote: 'onnx/model_fp16.onnx',
    local: 'model_fp16.onnx',
    sha256: 'cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04',
  },
  {
    remote: 'tokenizer.json',
    local: 'tokenizer.json',
    sha256: 'd241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66',
  },
  {
    remote: 'config.json',
    local: 'config.json',
    sha256: '9ab00bd92cee80a569f708140b7b6c1661a65891ff3765b1519e181ba2f2c92b',
  },
  {
    remote: 'special_tokens_map.json',
    local: 'special_tokens_map.json',
    sha256: '5d5b662e421ea9fac075174bb0688ee0d9431699900b90662acd44b2a350503a',
  },
  {
    remote: 'tokenizer_config.json',
    local: 'tokenizer_config.json',
    sha256: 'd7e0000bcc80134debd2222220427e6bf5fa20a669f40a0d0d1409cc18e0a9bc',
  },
];

const DEST_ROOT = path.resolve(__dirname, '..', 'src-tauri', 'models', 'embeddings');

// A \r-redrawn progress bar is meaningful on a terminal and is pure noise
// anywhere else. On a pipe every redraw becomes its OWN log line — the 105 MB
// model emitted ~7,300 of them per run. Cosmetic, but CI logs should be
// readable. Print a single summary line per file when stdout is not a terminal.
const SHOW_PROGRESS = process.stdout.isTTY === true && !process.env.CI;

// See download-ort.cjs for the full measurement. Node leaves HTTPS keep-alive
// sockets pooled, and an idle TLS socket holds the event loop open long after
// the last byte arrives: this script's real work is ~5s and it then sat for a
// dead-flat 240.00s (+/-0.01s across six CI runs) doing nothing at all. Drop
// the pooled sockets once the downloads are done so the process can exit.
function closeIdleConnections() {
  https.globalAgent.destroy();
  require('http').globalAgent.destroy();
}

function downloadBuffer(url) {
  return new Promise((resolve, reject) => {
    const follow = (u, redirects = 0) => {
      if (redirects > 5) return reject(new Error('Too many redirects'));
      const mod = u.startsWith('https') ? https : require('http');
      mod.get(u, { headers: { 'User-Agent': '4DA-build' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          let next = res.headers.location;
          if (next.startsWith('/')) {
            const parsed = new URL(u);
            next = `${parsed.protocol}//${parsed.host}${next}`;
          }
          return follow(next, redirects + 1);
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

function sha256Of(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

async function main() {
  const force = process.argv.includes('--force');

  console.log('Embedding Model Bundler - nomic-embed-text v1.5 (fp16 ONNX, 768-dim)');
  console.log(`Repo: ${MODEL_REPO} @ ${REVISION}`);
  console.log(`Destination: ${DEST_ROOT}\n`);

  for (const legacy of LEGACY_DIR_NAMES) {
    const dir = path.join(DEST_ROOT, legacy);
    if (fs.existsSync(dir)) {
      fs.rmSync(dir, { recursive: true, force: true });
      console.log(`  removed retired bundle: ${legacy}`);
    }
  }

  const modelDir = path.join(DEST_ROOT, MODEL_DIR_NAME);
  fs.mkdirSync(modelDir, { recursive: true });

  let totalSize = 0;

  for (const { remote, local, sha256 } of FILES) {
    const dest = path.join(modelDir, local);

    if (!force && fs.existsSync(dest) && fs.statSync(dest).size > 0) {
      const existing = fs.readFileSync(dest);
      if (!sha256 || sha256Of(existing) === sha256) {
        const mb = (existing.length / 1048576).toFixed(1);
        console.log(`  ${local}: already cached (${mb}MB) - skipping`);
        totalSize += existing.length;
        continue;
      }
      console.log(`  ${local}: cached copy fails its checksum - re-downloading`);
    }

    const url = `https://huggingface.co/${MODEL_REPO}/resolve/${REVISION}/${remote}`;
    console.log(`  ${local}: downloading...`);

    // Retry transient network failures (e.g. ECONNRESET mid-download) so a
    // single blip doesn't fail the whole signed release.
    let buffer;
    for (let attempt = 1; ; attempt++) {
      try {
        buffer = await downloadBuffer(url);
        break;
      } catch (e) {
        if (attempt >= 4) throw e;
        console.error(`  ${local}: download attempt ${attempt}/4 failed (${e.message}); retrying...`);
        await new Promise((r) => setTimeout(r, 2000 * attempt));
      }
    }
    if (sha256) {
      const got = sha256Of(buffer);
      if (got !== sha256) {
        throw new Error(`${local}: SHA-256 mismatch (expected ${sha256}, got ${got})`);
      }
    }
    fs.writeFileSync(dest, buffer);

    const mb = (buffer.length / 1048576).toFixed(2);
    console.log(`  ${local}: saved (${mb}MB)${sha256 ? ', checksum verified' : ''}`);
    totalSize += buffer.length;
  }

  const totalMb = (totalSize / 1048576).toFixed(1);
  console.log(`\nDone. Embedding model ready for Tauri bundling (${totalMb}MB total).`);
}

main()
  .then(closeIdleConnections)
  .catch((err) => {
    console.error(`\nFATAL: ${err.message}`);
    process.exit(1);
  });
