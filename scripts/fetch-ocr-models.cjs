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
// FORMAT: these are the ONNX exports of the ocrs models. Until 2026-10-27 this
// script fetched the `.rten` variants; upstream deprecated those and put their
// URLs into a brownout (HTTP 403 since 2026-10-27, robertknight/ocrs#282),
// which broke every fresh-clone install. rten 0.26 loads `.onnx` natively
// (its default `onnx_format` feature, which src-tauri/Cargo.toml keeps on), and
// `Model::load_file` picks the format from the file extension, so the file
// names below are load-bearing. Upstream also asks consumers to host or bundle
// their own copies long-term (robertknight/ocrs#283); until 4DA does, the pins
// below are what keep a moved or replaced upstream file from reaching a build.
// ONNX pins recorded 2026-10-28.
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
    file: "text-detection.onnx",
    url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx",
    sha256: "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe",
    bytes: 2499479,
  },
  {
    file: "text-recognition.onnx",
    url: "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx",
    sha256: "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5",
    bytes: 9713177,
  },
];

// The `.rten` files this script fetched before the switch to ONNX. Nothing
// loads them any more; main() deletes them from MODELS_DIR so an existing
// checkout does not keep 12 MB of dead weights, and release-ocr-models.cjs
// strips them from a release bundle along with MODELS.
const LEGACY_FILES = ["text-detection.rten", "text-recognition.rten"];

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

  for (const legacy of LEGACY_FILES) {
    const stale = path.join(MODELS_DIR, legacy);
    if (fs.existsSync(stale)) {
      fs.rmSync(stale, { force: true });
      console.log(`[ocr-models] removed ${legacy} (superseded by the ONNX model)`);
    }
  }

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

module.exports = { MODELS, LEGACY_FILES, MODELS_DIR, sha256File, verify };
