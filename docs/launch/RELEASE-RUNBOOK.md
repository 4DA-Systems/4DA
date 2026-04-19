# Release Runbook

Step-by-step procedure for cutting a 4DA release. `scripts/release.sh` is the automated gate; this document is the human-runnable playbook that wraps it and covers the steps that must happen outside the gate (signing, uploads, announcements).

**Before starting any release:** have a clean `main` checkout, a working network connection to the signing service, and a Windows machine (NSIS is the primary bundle vehicle). If any of these aren't true, stop.

---

## 1. Pre-release gate (automated)

```
./scripts/release.sh
```

The gate runs 9 steps: git hygiene, full test suite (frontend + Rust), validation suite, sovereignty score, cadence checks, version bumps, build, test count recording. **If any step returns non-zero, stop and investigate — do not bypass with `--skip-checks` unless you have read the specific failure and understand it.**

Expected runtime: 8–15 minutes on a warm checkout.

Artifacts the gate produces:
- `src-tauri/target/release/bundle/nsis/4DA Home_<version>_x64-setup.exe` (primary ship artifact)
- Test-count entry in `.claude/wisdom/ops-state.json` (trend tracking)

---

## 2. Artifact smoke test (automated)

```
node scripts/verify-installer.cjs
```

Verifies the installer is well-formed (PE header, size, SHA-256). On Windows also runs `Get-AuthenticodeSignature` — this will report **NotSigned** on a dev build, which is expected until step 4.

For a dev-loop check, pass `--unsigned-ok`. For a real release cut, **do not** pass it — the signature check should pass cleanly after step 4.

---

## 3. Release notes

Draft in the PR description or in a short file under `.claude/plans/` (gitignored). Cover:

1. User-visible changes (new features, UX fixes).
2. Security / privacy changes (this is a privacy-first product — always surface these).
3. Upgrade notes (anything that changes on-disk state, settings shape, keychain use).
4. Known limitations for this release — pull from `docs/launch/HONEST-ASSESSMENT-*.md`.

Keep it under 400 words. Paste into the GitHub Release body at step 6.

---

## 4. Code signing (manual — requires SSL.com CodeSignTool)

See `docs/launch/WINDOWS-INSTALL.md` for the signing toolchain setup. Short version:

```
./scripts/codesign-installer.sh <path-to-unsigned-installer>
```

Produces a signed `.exe` in the same directory. Re-run `verify-installer.cjs` without `--unsigned-ok` and confirm Authenticode reports **Valid**.

Record the signed installer's SHA-256 somewhere you'll still have in an hour — you need it for the release notes and for the verification page on 4da.ai.

---

## 5. Manual VM install test (non-negotiable)

The artifact-level checks cannot tell you whether the installer actually works. This step requires a clean Windows VM snapshot.

1. Revert VM to "fresh Windows 10/11" snapshot (no prior 4DA install).
2. Copy the signed installer over.
3. Double-click. Verify SmartScreen reports the publisher (post-signing). Install to the default location.
4. Launch the app from the Start menu.
5. Confirm:
   - App opens without error.
   - First-run flow displays correctly.
   - No API keys are required to see *something* (the "zero-config value" promise).
   - Settings → About shows the correct version.
   - Settings → Privacy shows activity-tracking OFF by default.
6. Close the app. Uninstall via Settings → Apps. Verify the install dir is gone.

**If any of 3–6 fails, do not tag the release.** Investigate, fix, rebuild, start over from step 1.

---

## 6. Tag and upload

```
git tag -a v<X.Y.Z> -m "Release v<X.Y.Z>"
git push origin v<X.Y.Z>
gh release create v<X.Y.Z> \
  --title "4DA v<X.Y.Z>" \
  --notes-file <release-notes.md> \
  "<signed-installer-path>#4DA Home x64 setup"
```

The GitHub Action for `post-release-packages.sh` will kick off automatically, producing updates for Homebrew cask, AUR, and winget manifests.

After upload: run `curl -I <download-url>` to confirm the installer is actually reachable at the release asset URL.

---

## 7. Post-release verification

Within 1 hour of the release going live:

- `curl -sSL <download-url> | sha256sum -c -` from a different network egress → must match the SHA-256 published in release notes.
- Homebrew cask update PR opens (automated).
- winget manifest PR opens (automated).
- 4da.ai download page shows the new version.

Within 24 hours:

- No anomalous crash reports from users (if Sentry is on, check the dashboard).
- No social-media complaints about a broken installer or antivirus false positive.

If any of these fail, consider a silent re-cut — yanking a release is worse than shipping two releases an hour apart.

---

## Related

- `docs/launch/DISTRIBUTION-CHECKLIST.md` — the multi-channel distribution plan.
- `docs/LAUNCH-ACTIONS.md` — human-only GitHub/npm/registrar tasks.
- `docs/launch/HONEST-ASSESSMENT-*.md` — current pre-launch state & known limitations.
- `.ai/FAILURE_MODES.md` — known fragile areas; read before investigating a weird release-time regression.
- `scripts/release.sh` — the automated gate invoked at step 1.
- `scripts/verify-installer.cjs` — the smoke test invoked at step 2.
