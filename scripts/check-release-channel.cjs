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

  // The workflow asking for latest.json is not the same as Tauri PRODUCING it.
  // `createUpdaterArtifacts` defaults to false, and with it unset the bundler
  // emitted no .sig at all, so "Found artifacts:" held only the .exe, the
  // required-asset check hard-failed, and the release stayed a draft. The gate
  // above checked the request and never the capability — text, not meaning.
  if (tauriConfig.bundle?.createUpdaterArtifacts !== true) {
    fail(
      'tauri.conf.json must set bundle.createUpdaterArtifacts: true — it defaults to false, ' +
        'and without it Tauri emits no updater signature, so latest.json can never be produced.'
    );
  }

  // A custom desktopTemplate opts out of Tauri's generated .desktop entry, so
  // the deep-link scheme in it is hand-maintained and drifted: the file still
  // registered `x-scheme-handler/4da` four months after #491 renamed the scheme
  // to `fourda`. Nothing referenced the file, which is why nothing caught it.
  const desktopTemplateRel = tauriConfig.bundle?.linux?.deb?.desktopTemplate;
  if (desktopTemplateRel) {
    const templatePath = path.join(root, 'src-tauri', desktopTemplateRel);
    if (!fs.existsSync(templatePath)) {
      fail(`tauri.conf.json references a desktopTemplate that does not exist: ${desktopTemplateRel}`);
    } else {
      const template = fs.readFileSync(templatePath, 'utf8');
      const schemes = tauriConfig.plugins?.['deep-link']?.desktop?.schemes ?? [];
      for (const scheme of schemes) {
        if (!template.includes(`x-scheme-handler/${scheme}`)) {
          fail(
            `${desktopTemplateRel} must register x-scheme-handler/${scheme} — ` +
              'the Linux packages are the only place this is hand-maintained.'
          );
        }
      }
      const stale = template.match(/x-scheme-handler\/([A-Za-z0-9+.-]+)/g) ?? [];
      for (const hit of stale) {
        const name = hit.split('/')[1];
        if (!schemes.includes(name)) {
          fail(
            `${desktopTemplateRel} registers x-scheme-handler/${name}, which is not in ` +
              'tauri.conf.json deep-link schemes. A retired scheme must not stay registered.'
          );
        }
      }
      // Without %U the handler launches with no argument, so the URL that
      // triggered it is dropped and activation silently does nothing.
      if (!/^Exec=.*%U/m.test(template)) {
        fail(`${desktopTemplateRel} Exec= must pass %U, or deep links arrive with no URL.`);
      }
    }
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

  // A `v*` tag is not necessarily a release — the repo has eight `v0.0.N-test`
  // dry-run tags. Both of the steps below used to be unconditional, so a dry run
  // that SUCCEEDED would have clobbered latest.json on the pointer release every
  // installed client polls, offering a test build to real users, and un-drafted
  // the test release. It never fired only because all eight dry runs failed
  // earlier in the matrix. The next release attempt is expected to open with
  // exactly such a dry run.
  const PRODUCTION_TAG_GUARD = "needs.create-release.outputs.production == 'true'";
  if (!releaseYml.includes('id: tagkind')) {
    fail('release.yml must classify the tag (id: tagkind) before publishing anything.');
  }
  if (!/\^v\[0-9\]\+\\.\[0-9\]\+\\.\[0-9\]\+\$/.test(releaseYml)) {
    fail('release.yml tag classifier must match a bare vMAJOR.MINOR.PATCH tag exactly.');
  }
  // The Windows signing waiver (ALLOW_UNSIGNED_WINDOWS) is the one deliberate
  // hole in a gate that exists because unsigned builds once shipped silently.
  // Three properties keep it honest, and all three are asserted here.
  //
  // 1. TAG-SCOPED, NOT BOOLEAN. The variable's value must equal the tag being
  //    built. A boolean left set to true authorises every future release; a tag
  //    name cannot, because the next tag differs and the gate closes again.
  if (releaseYml.includes('unsigned_windows_ok')) {
    const waiverLine = releaseYml
      .split(String.fromCharCode(10))
      .find((l) => l.includes('ALLOW_UNSIGNED_WINDOWS') && l.includes('"$TAG"'));
    if (!waiverLine) {
      fail(
        'release.yml must compare ALLOW_UNSIGNED_WINDOWS against the tag being built. ' +
          'A boolean waiver silently authorises every subsequent release.'
      );
    }

    // 2. WINDOWS ONLY. An un-notarized macOS .app is REFUSED by Gatekeeper, not
    //    warned about, so a waiver there ships a build nobody can open.
    const macCase = releaseYml.slice(releaseYml.indexOf('            macOS)'));
    const macBlock = macCase.slice(0, macCase.indexOf(';;'));
    if (/ALLOW_UNSIGNED|UNSIGNED_WINDOWS_OK/.test(macBlock)) {
      fail('the signing waiver must never apply to macOS — Gatekeeper refuses an un-notarized app outright.');
    }
    if (!/require APPLE_CERTIFICATE/.test(macBlock) || !/require APPLE_TEAM_ID/.test(macBlock)) {
      fail('release.yml must still require every Apple signing credential unconditionally.');
    }

    // 3. THE CHECK STILL RUNS. The waiver may change the verdict, never whether
    //    the measurement happens — that distinction is what #437 was about.
    const verifyIdx = releaseYml.indexOf('- name: Verify Windows Authenticode signature');
    const verifyHead = releaseYml.slice(verifyIdx, verifyIdx + 900);
    if (!verifyHead.includes("if: runner.os == \'Windows\'" + String.fromCharCode(10))) {
      fail(
        'the Windows signature check must stay unconditional on Windows. Skipping the ' +
          'check under a waiver recreates the 2026-04 defect: unsigned artifacts under a green tick.'
      );
    }
  }
  // npm publishes are irreversible, and publish-mcp had no tag classification
  // at all — a desktop dry-run tag would have attempted one. Gated at job level.
  const mcpJob = releaseYml.slice(releaseYml.indexOf('  publish-mcp:'));
  if (!releaseYml.includes('  publish-mcp:')) {
    fail('release.yml must contain the publish-mcp job.');
  } else if (!mcpJob.slice(0, 700).includes(PRODUCTION_TAG_GUARD)) {
    fail(
      'release.yml job "publish-mcp" must be gated on ' + PRODUCTION_TAG_GUARD +
        ' — an npm publish cannot be undone, and a dry-run tag must not trigger one.'
    );
  }

  for (const step of ['Publish desktop updater manifest', 'Publish release']) {
    const idx = releaseYml.indexOf(`- name: ${step}`);
    if (idx === -1) {
      fail(`release.yml must contain the "${step}" step.`);
      continue;
    }
    // The guard must be on the step itself: the next few lines, not anywhere.
    const window = releaseYml.slice(idx, idx + 200);
    if (!window.includes(PRODUCTION_TAG_GUARD)) {
      fail(
        `release.yml step "${step}" must be gated on ${PRODUCTION_TAG_GUARD} — ` +
          'otherwise a dry-run tag publishes to real users.'
      );
    }
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
