#!/usr/bin/env node
// Fetches the OCR models that the Tauri build declares as bundle resources
// (tauri.conf.json -> bundle.resources -> "models"). The directory is
// gitignored, so a fresh clone cannot compile src-tauri without this step --
// tauri-build hard-fails with `resource path \`models\` doesn't exist`.
// Runs from pnpm postinstall; skips instantly when the files are present.
// Same upstream as .github/workflows/release.yml (ocrs project model bucket).
//
// Non-fatal by design: an offline install still succeeds, with a clear
// pointer to the manual step, and the cargo error itself names the path.

const fs = require("fs");
const path = require("path");
const https = require("https");

const MODELS_DIR = path.join(__dirname, "..", "src-tauri", "models");
const MODELS = [
  { file: "text-detection.rten", url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten" },
  { file: "text-recognition.rten", url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten" },
];

function download(url, dest, redirectsLeft = 3) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { timeout: 60_000 }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location && redirectsLeft > 0) {
        res.resume();
        return resolve(download(res.headers.location, dest, redirectsLeft - 1));
      }
      if (res.statusCode !== 200) {
        res.resume();
        return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
      }
      const tmp = dest + ".tmp";
      const out = fs.createWriteStream(tmp);
      res.pipe(out);
      out.on("finish", () => {
        out.close(() => {
          fs.renameSync(tmp, dest);
          resolve();
        });
      });
      out.on("error", reject);
    });
    req.on("timeout", () => req.destroy(new Error(`timeout fetching ${url}`)));
    req.on("error", reject);
  });
}

async function main() {
  const missing = MODELS.filter((m) => !fs.existsSync(path.join(MODELS_DIR, m.file)));
  if (missing.length === 0) return; // common case: silent no-op

  fs.mkdirSync(MODELS_DIR, { recursive: true });
  for (const m of missing) {
    const dest = path.join(MODELS_DIR, m.file);
    try {
      process.stdout.write(`[ocr-models] fetching ${m.file}... `);
      await download(m.url, dest);
      const mb = (fs.statSync(dest).size / 1024 / 1024).toFixed(1);
      console.log(`done (${mb} MB)`);
    } catch (err) {
      try { fs.rmSync(dest + ".tmp", { force: true }); } catch {}
      console.warn(`FAILED (${err.message})`);
      console.warn(
        `[ocr-models] The Rust build needs src-tauri/models/${m.file}.\n` +
          `[ocr-models] Fetch it manually when online:\n` +
          `[ocr-models]   curl -sSL -o src-tauri/models/${m.file} ${m.url}`,
      );
    }
  }
}

main();
