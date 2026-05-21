/**
 * Download the fastembed embedding model for bundling with the Tauri installer.
 *
 * Fetches bge-small-en-v1.5 (quantized ONNX) from HuggingFace and creates
 * the hf-hub cache directory structure so fastembed recognizes the pre-cached
 * model and skips the first-run download entirely.
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

const MODEL_REPO = 'Qdrant/bge-small-en-v1.5-onnx-Q';
const CACHE_DIR_NAME = 'models--Qdrant--bge-small-en-v1.5-onnx-Q';
const COMMIT_HASH = '52398278842ec682c6f32300af41344b1c0b0bb2';

const FILES = [
  'model_optimized.onnx',
  'tokenizer.json',
  'config.json',
  'special_tokens_map.json',
  'tokenizer_config.json',
];

const DEST_ROOT = path.resolve(__dirname, '..', 'src-tauri', 'models', 'embeddings');
const SNAPSHOT_DIR = path.join(DEST_ROOT, CACHE_DIR_NAME, 'snapshots', COMMIT_HASH);
const REFS_DIR = path.join(DEST_ROOT, CACHE_DIR_NAME, 'refs');

function downloadFile(url) {
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

async function main() {
  const force = process.argv.includes('--force');

  console.log('Embedding Model Bundler — bge-small-en-v1.5 (quantized ONNX)');
  console.log(`Repo: ${MODEL_REPO}`);
  console.log(`Destination: ${DEST_ROOT}\n`);

  fs.mkdirSync(SNAPSHOT_DIR, { recursive: true });
  fs.mkdirSync(REFS_DIR, { recursive: true });

  // Write refs/main with the commit hash
  fs.writeFileSync(path.join(REFS_DIR, 'main'), COMMIT_HASH);

  let totalSize = 0;

  for (const file of FILES) {
    const dest = path.join(SNAPSHOT_DIR, file);

    if (!force && fs.existsSync(dest)) {
      const size = fs.statSync(dest).size;
      if (size > 0) {
        const mb = (size / 1048576).toFixed(1);
        console.log(`  ${file}: already cached (${mb}MB) — skipping`);
        totalSize += size;
        continue;
      }
    }

    const url = `https://huggingface.co/${MODEL_REPO}/resolve/main/${file}`;
    console.log(`  ${file}: downloading...`);

    const buffer = await downloadFile(url);
    fs.writeFileSync(dest, buffer);

    const mb = (buffer.length / 1048576).toFixed(2);
    console.log(`  ${file}: saved (${mb}MB)`);
    totalSize += buffer.length;
  }

  const totalMb = (totalSize / 1048576).toFixed(1);
  console.log(`\nDone. Embedding model ready for Tauri bundling (${totalMb}MB total).`);
}

main().catch((err) => {
  console.error(`\nFATAL: ${err.message}`);
  process.exit(1);
});
