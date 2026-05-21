/**
 * Download the fastembed embedding model for bundling with the Tauri installer.
 *
 * Fetches snowflake-arctic-embed-m (quantized ONNX, 768-dim) from HuggingFace
 * and creates the hf-hub cache directory structure so fastembed recognizes the
 * pre-cached model and skips the first-run download entirely.
 *
 * Usage:
 *   node scripts/download-embedding-model.cjs          # download model
 *   node scripts/download-embedding-model.cjs --force   # re-download even if cached
 *
 * Run before `cargo tauri build` alongside download-ort.cjs.
 */

'use strict';

const https = require('https');
const fs = require('fs');
const path = require('path');

const MODEL_REPO = 'Snowflake/snowflake-arctic-embed-m';
const CACHE_DIR_NAME = 'models--Snowflake--snowflake-arctic-embed-m';

const FILES = [
  { remote: 'onnx/model_quantized.onnx', local: 'onnx/model_quantized.onnx' },
  { remote: 'tokenizer.json', local: 'tokenizer.json' },
  { remote: 'config.json', local: 'config.json' },
  { remote: 'special_tokens_map.json', local: 'special_tokens_map.json' },
  { remote: 'tokenizer_config.json', local: 'tokenizer_config.json' },
];

const DEST_ROOT = path.resolve(__dirname, '..', 'src-tauri', 'models', 'embeddings');

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

function downloadText(url) {
  return new Promise((resolve, reject) => {
    const follow = (u, redirects = 0) => {
      if (redirects > 5) return reject(new Error('Too many redirects'));
      https.get(u, { headers: { 'User-Agent': '4DA-build' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return follow(res.headers.location, redirects + 1);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} for ${u}`));
        }
        let data = '';
        res.on('data', (c) => (data += c));
        res.on('end', () => resolve(data));
        res.on('error', reject);
      }).on('error', reject);
    };
    follow(url);
  });
}

async function resolveCommitHash() {
  const url = `https://huggingface.co/api/models/${MODEL_REPO}/revision/main`;
  const json = await downloadText(url);
  const { sha } = JSON.parse(json);
  if (!sha || sha.length < 40) {
    throw new Error(`Could not resolve commit hash from API: ${json.slice(0, 200)}`);
  }
  return sha;
}

async function main() {
  const force = process.argv.includes('--force');

  console.log('Embedding Model Bundler — snowflake-arctic-embed-m (quantized ONNX, 768-dim)');
  console.log(`Repo: ${MODEL_REPO}`);
  console.log(`Destination: ${DEST_ROOT}\n`);

  console.log('Resolving latest commit hash...');
  const commitHash = await resolveCommitHash();
  console.log(`Commit: ${commitHash}\n`);

  const snapshotDir = path.join(DEST_ROOT, CACHE_DIR_NAME, 'snapshots', commitHash);
  const refsDir = path.join(DEST_ROOT, CACHE_DIR_NAME, 'refs');

  fs.mkdirSync(snapshotDir, { recursive: true });
  fs.mkdirSync(refsDir, { recursive: true });

  fs.writeFileSync(path.join(refsDir, 'main'), commitHash);

  let totalSize = 0;

  for (const { remote, local } of FILES) {
    const dest = path.join(snapshotDir, local);
    const destDir = path.dirname(dest);
    fs.mkdirSync(destDir, { recursive: true });

    if (!force && fs.existsSync(dest)) {
      const size = fs.statSync(dest).size;
      if (size > 0) {
        const mb = (size / 1048576).toFixed(1);
        console.log(`  ${local}: already cached (${mb}MB) — skipping`);
        totalSize += size;
        continue;
      }
    }

    const url = `https://huggingface.co/${MODEL_REPO}/resolve/main/${remote}`;
    console.log(`  ${local}: downloading...`);

    const buffer = await downloadBuffer(url);
    fs.writeFileSync(dest, buffer);

    const mb = (buffer.length / 1048576).toFixed(2);
    console.log(`  ${local}: saved (${mb}MB)`);
    totalSize += buffer.length;
  }

  const totalMb = (totalSize / 1048576).toFixed(1);
  console.log(`\nDone. Embedding model ready for Tauri bundling (${totalMb}MB total).`);
}

main().catch((err) => {
  console.error(`\nFATAL: ${err.message}`);
  process.exit(1);
});
