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
// names below are load-bearing. ONNX pins recorded 2026-10-28.
//
// SOURCES: upstream asks consumers to host their own copies (robertknight/ocrs#282,
// #283), so 4DA mirrors the pinned bytes as assets on the `ocr-models-onnx-1`
// GitHub release (a prerelease, never "latest", on a tag release.yml does not
// match). Each model lists its candidate URLs in order: the 4DA mirror first,
// then upstream. A candidate is accepted only if its size and SHA-256 match the
// pin -- the mirror is trusted no more than upstream is. A candidate that fails
// either way is discarded and the next one is tried.
//
// FAILURE POLICY -- applied once every candidate has been tried:
//   * NETWORK failure  -> non-fatal by default. An offline install still
//                         succeeds, with a clear pointer to the manual step,
//                         and the cargo error itself names the path.
//   * CHECKSUM mismatch -> fatal when no candidate verified, and the bytes are
//                         deleted. "The bytes are not the bytes we pinned" is
//                         the exact condition this check exists to stop; a
//                         mismatched candidate is never kept, even when a later
//                         candidate is unreachable.
//   * --require          -> makes network failure fatal too. Used by
//                         release.yml, where shipping without verified models is
//                         not an option.

const fs = require("fs");
const path = require("path");
const https = require("https");
const crypto = require("crypto");

const MODELS_DIR = path.join(__dirname, "..", "src-tauri", "models");
const MIRROR_BASE = "https://github.com/4DA-Systems/4DA/releases/download/ocr-models-onnx-1";
const UPSTREAM_BASE = "https://ocrs-models.s3-accelerate.amazonaws.com";
const MODELS = [
  {
    file: "text-detection.onnx",
    urls: [`${MIRROR_BASE}/text-detection.onnx`, `${UPSTREAM_BASE}/text-detection.onnx`],
    sha256: "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe",
    bytes: 2499479,
  },
  {
    file: "text-recognition.onnx",
    urls: [`${MIRROR_BASE}/text-recognition.onnx`, `${UPSTREAM_BASE}/text-recognition.onnx`],
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

// GitHub release assets redirect twice (github.com -> release-assets host ->
// signed URL), so allow a little headroom.
function download(url, dest, redirectsLeft = 5) {
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

function removeQuietly(file) {
  try {
    fs.rmSync(file, { force: true });
  } catch {
    /* best effort */
  }
}

/**
 * Tries each of `model.urls` in order and keeps the first download that matches
 * the pin. A mismatched download is deleted before the next candidate is tried.
 *
 * @returns {Promise<{ok: true, url: string} |
 *   {ok: false, integrity: boolean, failures: Array<{url: string, reason: string}>}>}
 *   `integrity` is true when at least one candidate served bytes that did not
 *   match the pin (as opposed to every candidate being unreachable).
 */
async function fetchModel(model, dest, fetcher = download) {
  const failures = [];
  let integrity = false;
  for (const url of model.urls) {
    try {
      await fetcher(url, dest);
    } catch (err) {
      removeQuietly(dest + ".tmp");
      removeQuietly(dest);
      failures.push({ url, reason: err.message });
      continue;
    }
    const check = verify(dest, model);
    if (check.ok) {
      // An earlier source served the wrong bytes: the install still succeeds on
      // verified bytes, but the caller must surface it, never swallow it.
      const tampered = failures.filter((f) => f.reason.startsWith("INTEGRITY:"));
      return tampered.length ? { ok: true, url, tampered } : { ok: true, url };
    }
    removeQuietly(dest);
    integrity = true;
    failures.push({ url, reason: `INTEGRITY: ${check.reason}` });
  }
  return { ok: false, integrity, failures };
}

function reportIntegrityFailure(model, failures) {
  console.error(`[ocr-models] INTEGRITY FAILURE for ${model.file}: no source served the pinned bytes`);
  for (const f of failures) console.error(`[ocr-models]   ${f.url}: ${f.reason}`);
  console.error(`[ocr-models]   The downloaded bytes do not match the digest pinned in`);
  console.error(`[ocr-models]   scripts/fetch-ocr-models.cjs, so they were DELETED rather than`);
  console.error(`[ocr-models]   handed to the bundler. This is either an intentional upstream`);
  console.error(`[ocr-models]   model update (verify it, then update the pin) or tampering.`);
}

/** @returns {Promise<number>} the process exit code */
async function main(argv = process.argv, { models = MODELS, modelsDir = MODELS_DIR, fetcher = download } = {}) {
  const require_ = argv.includes("--require");
  fs.mkdirSync(modelsDir, { recursive: true });

  for (const legacy of LEGACY_FILES) {
    const stale = path.join(modelsDir, legacy);
    if (fs.existsSync(stale)) {
      fs.rmSync(stale, { force: true });
      console.log(`[ocr-models] removed ${legacy} (superseded by the ONNX model)`);
    }
  }

  for (const model of models) {
    const dest = path.join(modelsDir, model.file);

    // An already-present file is NOT trusted just for existing. The old script
    // skipped on existence alone, so a stale, truncated or substituted blob sat
    // in the tree and went straight into the signed bundle.
    if (fs.existsSync(dest)) {
      const check = verify(dest, model);
      if (check.ok) continue;
      console.warn(`[ocr-models] ${model.file} present but does not match the pin (${check.reason}) — refetching`);
      fs.rmSync(dest, { force: true });
    }

    process.stdout.write(`[ocr-models] fetching ${model.file}... `);
    const result = await fetchModel(model, dest, fetcher);
    if (result.ok) {
      const via = result.url === model.urls[0] ? "" : `, via fallback ${result.url}`;
      console.log(`done (${(model.bytes / 1024 / 1024).toFixed(1)} MB, SHA-256 verified${via})`);
      for (const f of result.tampered ?? []) {
        console.error(`[ocr-models] WARNING: ${f.url} served bytes that do not match the pin`);
        console.error(`[ocr-models]   (${f.reason}). They were deleted; investigate that source.`);
      }
      continue;
    }

    console.warn("FAILED");
    if (result.integrity) {
      reportIntegrityFailure(model, result.failures);
      return 1;
    }
    for (const f of result.failures) console.warn(`[ocr-models]   ${f.url}: ${f.reason}`);
    if (require_) {
      console.error(`[ocr-models] --require was set: a build that must ship cannot proceed without`);
      console.error(`[ocr-models] verified OCR models. Aborting.`);
      return 1;
    }
    console.warn(
      `[ocr-models] The Rust build needs src-tauri/models/${model.file}.\n` +
        `[ocr-models] Fetch it manually when online (either URL):\n` +
        model.urls.map((u) => `[ocr-models]   curl -sSL -o src-tauri/models/${model.file} ${u}\n`).join("") +
        `[ocr-models]   and verify: sha256sum src-tauri/models/${model.file}\n` +
        `[ocr-models]   expected: ${model.sha256}`,
    );
  }
  return 0;
}

if (require.main === module) {
  main().then((code) => {
    process.exitCode = code;
  });
}

module.exports = { MODELS, LEGACY_FILES, MODELS_DIR, MIRROR_BASE, UPSTREAM_BASE, sha256File, verify, fetchModel, main };
