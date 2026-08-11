#!/usr/bin/env node
// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const fs = require('node:fs');
const path = require('node:path');

const EXPECTED_ENDPOINT =
  'https://github.com/4DA-Systems/4DA/releases/download/desktop-latest/latest.json';
const EXPECTED_DESKTOP_VERSION_LINE = '1.0';

function read(root, relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function readJson(root, relativePath) {
  return JSON.parse(read(root, relativePath));
}

function cargoPackageVersion(cargoToml) {
  const start = cargoToml.indexOf('[package]');
  const rest = start >= 0 ? cargoToml.slice(start + '[package]'.length) : cargoToml;
  const nextSection = rest.search(/\n\[/);
  const source = nextSection >= 0 ? rest.slice(0, nextSection) : rest;
  return source.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? null;
}

function cargoLockPackageVersion(cargoLock, packageName) {
  const packageBlocks = cargoLock.split(/\n\[\[package\]\]\n/g);
  for (const block of packageBlocks) {
    if (block.match(new RegExp(`^name\\s*=\\s*"${packageName}"`, 'm'))) {
      return block.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? null;
    }
  }
  return null;
}

function migrationTargetVersion(migrationsRs) {
  const match = migrationsRs.match(/const\s+TARGET_VERSION:\s*i64\s*=\s*(\d+)\s*;/);
  return match ? Number(match[1]) : null;
}

function isExpectedDesktopVersion(version) {
  return /^1\.0\.[1-9]\d*$/.test(version);
}

function checkReleaseChannel(root = path.resolve(__dirname, '..')) {
  const errors = [];
  const fail = (message) => errors.push(message);

  const packageJson = readJson(root, 'package.json');
  const tauriConfig = readJson(root, 'src-tauri/tauri.conf.json');
  const cargoToml = read(root, 'src-tauri/Cargo.toml');
  const cargoLock = read(root, 'src-tauri/Cargo.lock');
  const migrationsRs = read(root, 'src-tauri/src/db/migrations.rs');
  const releaseYml = read(root, '.github/workflows/release.yml');
  const codeSignPinHelper = read(root, 'scripts/pin-codesigntool-sha.sh');
  const mcpbWorkflow = read(root, '.github/workflows/build-mcpb-extensions.yml');
  const networkTransparency = read(root, 'docs/NETWORK-TRANSPARENCY.md');
  const securityAuditGuide = read(root, 'docs/SECURITY-AUDIT-GUIDE.md');

  const cargoVersion = cargoPackageVersion(cargoToml);
  const cargoLockVersion = cargoLockPackageVersion(cargoLock, 'fourda');
  const versions = {
    'package.json': packageJson.version,
    'tauri.conf.json': tauriConfig.version,
    'Cargo.toml': cargoVersion,
    'Cargo.lock': cargoLockVersion,
  };

  const uniqueVersions = new Set(Object.values(versions));
  if (uniqueVersions.size !== 1 || uniqueVersions.has(null) || uniqueVersions.has(undefined)) {
    fail(`App versions must match across package.json, tauri.conf.json, Cargo.toml, and Cargo.lock: ${JSON.stringify(versions)}`);
  }
  if (!isExpectedDesktopVersion(packageJson.version)) {
    fail(`Desktop app must stay on the ${EXPECTED_DESKTOP_VERSION_LINE}.x hardening line before the next maturity release; use ${EXPECTED_DESKTOP_VERSION_LINE}.1, ${EXPECTED_DESKTOP_VERSION_LINE}.2, etc. and do not publish 1.1.0 yet.`);
  }

  const targetVersion = migrationTargetVersion(migrationsRs);
  if (!Number.isInteger(targetVersion)) {
    fail('Could not find database TARGET_VERSION in src-tauri/src/db/migrations.rs.');
  } else if (targetVersion > 59 && packageJson.version === '1.0.0') {
    fail(`Database TARGET_VERSION is ${targetVersion}, but app version is still 1.0.0; desktop releases must bump semver when schema compatibility advances.`);
  }

  const endpoints = tauriConfig.plugins?.updater?.endpoints ?? [];
  if (!Array.isArray(endpoints) || endpoints.length !== 1 || endpoints[0] !== EXPECTED_ENDPOINT) {
    fail(`Updater endpoint must be the desktop-only manifest pointer: ${EXPECTED_ENDPOINT}`);
  }

  const searchedFiles = {
    'src-tauri/tauri.conf.json': JSON.stringify(tauriConfig),
    'docs/NETWORK-TRANSPARENCY.md': networkTransparency,
    'docs/SECURITY-AUDIT-GUIDE.md': securityAuditGuide,
    '.github/workflows/release.yml': releaseYml,
  };
  for (const [file, contents] of Object.entries(searchedFiles)) {
    if (contents.includes('/releases/latest/download/latest.json')) {
      fail(`${file} still points at GitHub's global releases/latest updater endpoint.`);
    }
  }

  if (!networkTransparency.includes(EXPECTED_ENDPOINT)) {
    fail('docs/NETWORK-TRANSPARENCY.md must document the desktop updater endpoint.');
  }
  if (!securityAuditGuide.includes(EXPECTED_ENDPOINT)) {
    fail('docs/SECURITY-AUDIT-GUIDE.md must document the desktop updater endpoint.');
  }

  if (!releaseYml.includes('includeUpdaterJson: true')) {
    fail('release.yml must set includeUpdaterJson: true so Tauri emits latest.json.');
  }
  const requiredAssets = releaseYml.match(/REQUIRED=\(([\s\S]*?)\n\s*\)/)?.[1] ?? '';
  if (!requiredAssets.includes('"latest.json"')) {
    fail('release.yml must require latest.json before publishing a desktop release.');
  }
  if (releaseYml.includes('::warning::latest.json')) {
    fail('release.yml must not downgrade a missing latest.json to a warning.');
  }
  if (!releaseYml.includes('Publish desktop updater manifest')) {
    fail('release.yml must publish latest.json to the desktop-latest pointer release.');
  }
  if (!releaseYml.includes('POINTER_TAG="desktop-latest"')) {
    fail('release.yml must use the desktop-latest pointer tag for updater manifests.');
  }
  if (!releaseYml.includes('gh release upload "$POINTER_TAG" "$WORK_DIR/latest.json"')) {
    fail('release.yml must upload latest.json to the desktop-latest release.');
  }
  if (releaseYml.includes('PLACEHOLDER_SHA256') || releaseYml.includes('TODO: compute the SHA-256')) {
    fail('release.yml must not contain placeholder CodeSignTool checksum instructions.');
  }
  const codeSignToolHash = releaseYml.match(/\$expected\s*=\s*"([0-9a-fA-F]+)"/)?.[1] ?? null;
  if (!codeSignToolHash || !/^[0-9a-fA-F]{64}$/.test(codeSignToolHash)) {
    fail('release.yml must pin SSL.com CodeSignTool with a 64-character SHA-256 hash.');
  }
  if (!releaseYml.includes('Get-FileHash -Path $zipPath -Algorithm SHA256')) {
    fail('release.yml must compute SHA-256 for the downloaded CodeSignTool archive.');
  }
  if (!/if\s*\(\$actual\s+-ne\s+\$expected\.ToLowerInvariant\(\)\)\s*\{[^}]*CodeSignTool SHA-256 mismatch/s.test(releaseYml)) {
    fail('release.yml must fail when the downloaded CodeSignTool hash differs from the pinned hash.');
  }
  if (codeSignPinHelper.includes('PLACEHOLDER_SHA256_FILL_IN') || codeSignPinHelper.includes('TODO: compute the SHA-256')) {
    fail('pin-codesigntool-sha.sh must update the current pinned hash, not a placeholder checksum.');
  }
  if (!codeSignPinHelper.includes('grep -Eq \'\\$expected = "[0-9a-fA-F]{64}"\' "$WORKFLOW"')) {
    fail('pin-codesigntool-sha.sh must locate the current pinned 64-character SHA-256 before replacing it.');
  }
  if (!codeSignPinHelper.includes('-E "s|\\\\\\$expected = \\"[0-9a-fA-F]{64}\\"|\\\\\\$expected = \\"$SHA_LC\\"|"')) {
    fail('pin-codesigntool-sha.sh must replace the current pinned CodeSignTool SHA-256.');
  }

  if (!mcpbWorkflow.includes('--prerelease')) {
    fail('MCP .mcpb releases must be marked prerelease so they cannot become GitHub global latest.');
  }
  if (!mcpbWorkflow.includes('gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --prerelease')) {
    fail('Existing MCP .mcpb releases must be edited to prerelease on every bundle publish.');
  }

  return errors;
}

if (require.main === module) {
  const errors = checkReleaseChannel();
  if (errors.length > 0) {
    console.error('Release-channel check failed:');
    for (const error of errors) {
      console.error(`  - ${error}`);
    }
    process.exit(1);
  }
  console.log('Release-channel check passed.');
}

module.exports = {
  EXPECTED_DESKTOP_VERSION_LINE,
  EXPECTED_ENDPOINT,
  checkReleaseChannel,
};
