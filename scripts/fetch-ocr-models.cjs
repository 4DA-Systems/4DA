#!/usr/bin/env node
// Fetches the OCR models that the Tauri build declares as bundle resources
// (tauri.conf.json -> bundle.resources -> "models"). The directory is
// gitignored, so a fresh clone cannot compile src-tauri without this step --
// tauri-build hard-fails with `resource path \`models\` doesn't exist`.
// Runs from pnpm postinstall; verifies instantly when the files are present.
//
// SUPPLY CHAIN: these two blobs are downloaded over the network from a
// third-party S3 bucket and then bundled INTO the signed, notarized installer
// by .github/workflows/release.yml. Until 2026-08-16 they arrived with no
// integrity check of any kind, which meant a bucket compromise, a hijacked
// redirect or a truncated transfer put unverified bytes inside an artifact
// carrying 4DA's EV signature -- the signature attesting to content nobody had
// checked. The same workflow already SHA-256-pins CodeSignTool and hard-fails on
// mismatch; this closes the gap for the models.
//
// The pinned digests below were computed by downloading each file and hashing
// it (see the sizes, which are also asserted). Refresh them ONLY after
// deliberately verifying an intentional upstream model update.
//
// FAILURE POLICY -- the two cases are deliberately different:
//   * NETWORK failure  -> non-fatal by default. An offline install still
//                         succeeds, with a clear pointer to the manual step,
//                         and the cargo error itself names the path.
//   * CHECKSUM mismatch -> ALWAYS fatal, and the file is deleted. "The bytes are
//                         not the bytes we pinned" is the exact condition this
//                         check exists to stop; degrading it to a warning would
//                         make the check decorative.
//   * --require          -> makes network failure fatal too. Used by
//                         release.yml, where shipping without verified models is
//                         not an option.

const fs = require("fs");
const path = require("path");
const https = require("https");
const crypto = require("crypto");

const MODELS_DIR = path.join(__dirname, "..", "src-tauri", "models");
const MODELS = [
  {
    file: "text-detection.rten",
    url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten",
    sha256: "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca",
    bytes: 2510284,
  },
  {
    file: "text-recognition.rten",
    url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten",
    sha256: "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e",
    bytes: 9716568,
  },
];

function sha256File(file) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(file));
  return hash.digest("hex");
}

/**
 * @returns {{ok: true} | {ok: false, reason: string}}
 */
function verify(file, model) {
  let size;
  try {
    size = fs.statSync(file).size;
  } catch (err) {
    return { ok: false, reason: `cannot stat: ${err.message}` };
  }
  if (size !== model.bytes) {
    return { ok: false, reason: `size ${size} != pinned ${model.bytes}` };
  }
  const actual = sha256File(file);
  if (actual !== model.sha256) {
    return { ok: false, reason: `SHA-256 ${actual} != pinned ${model.sha256}` };
  }
  return { ok: true };
}

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

function tamperedExit(model, dest, reason) {
  try {
    fs.rmSync(dest, { force: true });
  } catch {
    /* best effort — the message below is what matters */
  }
  console.error(`[ocr-models] INTEGRITY FAILURE for ${model.file}: ${reason}`);
  console.error(`[ocr-models]   source: ${model.url}`);
  console.error(`[ocr-models]   The downloaded bytes do not match the digest pinned in`);
  console.error(`[ocr-models]   scripts/fetch-ocr-models.cjs, so they were DELETED rather than`);
  console.error(`[ocr-models]   handed to the bundler. This is either an intentional upstream`);
  console.error(`[ocr-models]   model update (verify it, then update the pin) or tampering.`);
  process.exit(1);
}

async function main() {
  const require_ = process.argv.includes("--require");
  fs.mkdirSync(MODELS_DIR, { recursive: true });

  for (const model of MODELS) {
    const dest = path.join(MODELS_DIR, model.file);

    // An already-present file is NOT trusted just for existing. The old script
    // skipped on existence alone, so a stale, truncated or substituted blob sat
    // in the tree and went straight into the signed bundle.
    if (fs.existsSync(dest)) {
      const check = verify(dest, model);
      if (check.ok) continue;
      console.warn(`[ocr-models] ${model.file} present but does not match the pin (${check.reason}) — refetching`);
      fs.rmSync(dest, { force: true });
    }

    try {
      process.stdout.write(`[ocr-models] fetching ${model.file}... `);
      await download(model.url, dest);
    } catch (err) {
      try {
        fs.rmSync(dest + ".tmp", { force: true });
      } catch {
        /* best effort */
      }
      console.warn(`FAILED (${err.message})`);
      if (require_) {
        console.error(`[ocr-models] --require was set: a build that must ship cannot proceed without`);
        console.error(`[ocr-models] verified OCR models. Aborting.`);
        process.exit(1);
      }
      console.warn(
        `[ocr-models] The Rust build needs src-tauri/models/${model.file}.\n` +
          `[ocr-models] Fetch it manually when online:\n` +
          `[ocr-models]   curl -sSL -o src-tauri/models/${model.file} ${model.url}\n` +
          `[ocr-models]   and verify: sha256sum src-tauri/models/${model.file}\n` +
          `[ocr-models]   expected: ${model.sha256}`,
      );
      continue;
    }

    const check = verify(dest, model);
    if (!check.ok) tamperedExit(model, dest, check.reason);
    console.log(`done (${(model.bytes / 1024 / 1024).toFixed(1)} MB, SHA-256 verified)`);
  }
}

if (require.main === module) {
  main();
}

module.exports = { MODELS, MODELS_DIR, sha256File, verify };
