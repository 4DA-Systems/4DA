#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Pin the desktop updater manifest (latest.json) to its own release tag, and
// prove its payload URLs answer (audit 2026-11-11, R5).
//
// tauri-action writes every payload URL as
//   https://github.com/<repo>/releases/latest/download/<file>
// which resolves through GitHub's "Latest release" flag, not through the
// release the manifest belongs to. Until the un-draft lands that flag still
// names the previous release, whose assets have different file names, and
// any later change of the flag (an MCP release marked latest, a re-ordered
// release) silently points every installed app at the wrong payload. Pinned:
//   https://github.com/<repo>/releases/download/<tag>/<file>
//
// Usage (release.yml, publish-release job):
//   node scripts/pin-updater-manifest.cjs pin <latest.json> --repo <o/r> --tag <vX.Y.Z> --assets <names.txt>
//       rewrites the file in place; fails if an entry is not a GitHub release
//       URL, or names a file the release does not carry.
//   node scripts/pin-updater-manifest.cjs check-urls <latest.json> [--attempts N] [--delay-ms N]
//       GETs every payload URL (following redirects) and fails unless each
//       answers 200; run AFTER the release is public (a draft's assets 404).

'use strict';

const fs = require('fs');

const LATEST_RE = /^https:\/\/github\.com\/([^/]+\/[^/]+)\/releases\/latest\/download\/([^/?#]+)$/;
const PINNED_RE = /^https:\/\/github\.com\/([^/]+\/[^/]+)\/releases\/download\/([^/?#]+)\/([^/?#]+)$/;

/**
 * Rewrite every platform URL to `/releases/download/<tag>/<file>`.
 * Returns `{ manifest, problems }`; the input is not modified.
 */
function pinManifest(manifest, repo, tag, assetNames) {
  const problems = [];
  const assets = new Set(assetNames);
  const platforms = manifest && typeof manifest.platforms === 'object' ? manifest.platforms : null;
  if (!platforms || Object.keys(platforms).length === 0) {
    return { manifest, problems: ['latest.json has no platforms'] };
  }
  if (manifest.version && `v${manifest.version}` !== tag) {
    problems.push(`latest.json version ${manifest.version} does not match tag ${tag}`);
  }
  const out = { ...manifest, platforms: {} };
  for (const [key, entry] of Object.entries(platforms)) {
    const url = String((entry && entry.url) || '');
    let file = null;
    let m = url.match(LATEST_RE);
    if (m && m[1] === repo) file = m[2];
    else if ((m = url.match(PINNED_RE)) && m[1] === repo && m[2] === tag) file = m[3];
    if (!file) {
      problems.push(`${key}: ${url || '(no url)'} is not a ${repo} release URL for ${tag}`);
      out.platforms[key] = entry;
      continue;
    }
    if (!assets.has(decodeURIComponent(file))) {
      problems.push(`${key}: ${file} is not an asset of ${tag}`);
    }
    out.platforms[key] = { ...entry, url: `https://github.com/${repo}/releases/download/${tag}/${file}` };
  }
  return { manifest: out, problems };
}

/** Every distinct payload URL in a manifest. */
function payloadUrls(manifest) {
  return [...new Set(Object.values(manifest.platforms || {}).map((e) => e && e.url).filter(Boolean))];
}

/** GET a URL (redirects followed); the final status, or the error text. */
async function probe(url, fetchImpl) {
  try {
    const res = await fetchImpl(url, { method: 'GET', redirect: 'follow' });
    // Do not download the payload: the status line is the answer.
    if (res.body && typeof res.body.cancel === 'function') await res.body.cancel().catch(() => undefined);
    return { url, status: res.status };
  } catch (err) {
    return { url, status: 0, error: err && err.message ? err.message : String(err) };
  }
}

/** Probe every URL, retrying non-200s (GitHub's CDN can lag an un-draft by seconds). */
async function checkUrls(urls, { attempts = 5, delayMs = 10_000, fetchImpl = fetch, sleep } = {}) {
  const wait = sleep || ((ms) => new Promise((r) => setTimeout(r, ms)));
  let pending = urls.slice();
  let results = [];
  for (let attempt = 1; attempt <= attempts && pending.length; attempt++) {
    results = await Promise.all(pending.map((u) => probe(u, fetchImpl)));
    pending = results.filter((r) => r.status !== 200).map((r) => r.url);
    if (pending.length && attempt < attempts) await wait(delayMs);
  }
  const failed = results.filter((r) => r.status !== 200);
  return { ok: failed.length === 0, failed };
}

function arg(argv, name, fallback) {
  const i = argv.indexOf(name);
  return i >= 0 && argv[i + 1] !== undefined ? argv[i + 1] : fallback;
}

async function main(argv) {
  const [mode, file] = argv;
  if (!mode || !file) {
    console.error('usage: pin-updater-manifest.cjs pin|check-urls <latest.json> [...]');
    return 2;
  }
  const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));

  if (mode === 'pin') {
    const repo = arg(argv, '--repo');
    const tag = arg(argv, '--tag');
    const assetsFile = arg(argv, '--assets');
    if (!repo || !tag || !assetsFile) {
      console.error('pin needs --repo, --tag and --assets');
      return 2;
    }
    const names = fs.readFileSync(assetsFile, 'utf8').split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
    const { manifest: pinned, problems } = pinManifest(manifest, repo, tag, names);
    if (problems.length) {
      problems.forEach((p) => console.error(`::error::${p}`));
      return 1;
    }
    fs.writeFileSync(file, JSON.stringify(pinned, null, 2) + '\n');
    payloadUrls(pinned).forEach((u) => console.log(`pinned: ${u}`));
    return 0;
  }

  if (mode === 'check-urls') {
    const urls = payloadUrls(manifest);
    if (urls.length === 0) {
      console.error('::error::latest.json names no payload URLs');
      return 1;
    }
    const { ok, failed } = await checkUrls(urls, {
      attempts: Number(arg(argv, '--attempts', 5)),
      delayMs: Number(arg(argv, '--delay-ms', 10_000)),
    });
    for (const u of urls) {
      const f = failed.find((x) => x.url === u);
      console.log(`${f ? `FAIL ${f.status}${f.error ? ` (${f.error})` : ''}` : 'OK 200'}  ${u}`);
    }
    if (!ok) {
      console.error(`::error::${failed.length} updater payload URL(s) do not answer 200. Installed apps would fail to update.`);
      return 1;
    }
    return 0;
  }

  console.error(`unknown mode ${mode}`);
  return 2;
}

module.exports = { pinManifest, payloadUrls, checkUrls };

if (require.main === module) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
