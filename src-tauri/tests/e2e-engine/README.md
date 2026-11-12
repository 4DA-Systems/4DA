# Engine end-to-end gate

`scripts/ci/engine-e2e.mjs` runs the real `fourda-engine` binary the way a user's
machine runs it and asserts on the database it leaves behind. CI job: `Engine
end-to-end` in `.github/workflows/validate.yml` (merge-blocking through `Validate
Success`). Audit 2026-11-11, finding Q1: before this, nothing automated ran the
product.

## What it does

1. Copies the conformance cases named in `gate.json` (`../conformance/cases/<id>`,
   without `case.json`) into a fresh context directory and writes a fresh profile
   (`settings.json` from `data/settings.example.json`: no LLM, onboarding done).
2. Builds a frozen advisory mirror: every pinned OSV record in `../conformance/osv/`
   (not the synthetic `SYNT-*` rules), one `all.zip` per ecosystem, served with an
   ETag from `127.0.0.1`. The engine reads it through `FOURDA_E2E_OSV_MIRROR`, a hook
   that exists only in a debug build with `--features e2e-osv-mirror`
   (`src/e2e_gate.rs`; a release build with the feature does not compile).
3. Blocks the network: every proxy variable points at a closed port and `NO_PROXY`
   exempts only `127.0.0.1`. `FOURDA_E2E_SKIP_FEED_FETCH=1` skips cycle step 1 (news
   and registry feeds), which with no network only spends its retry budget
   (3 x 90 s per adapter, measured 2026-11-12); the dependency path runs unchanged.
4. Runs `fourda-engine --once --force` three times:

| run | setup | asserted |
|---|---|---|
| `run1` | fresh profile, mirror up | exit 0; inventory of every case == `../conformance/expected/<id>.json`, minus the directories the coverage record names as skipped; every truth finding's advisory (or an alias) stored for its package; every synced ecosystem verified (error NULL); the coverage record == `golden/coverage.json`; the plan snapshot == `golden/plan-run1.json`; the archives were fetched from the mirror |
| `run2` | `mutation` applied to one lockfile (a user applying a fix) | exit 0; the inventory follows the change; findings on the removed version are gone; every ecosystem verified; plan == `golden/plan-run2.json` |
| `run3-offline` | the mirror stopped | exit 1; every ecosystem records "could not check for newer advisories" and keeps its last verified time; the `engine_runs` receipt is a failed run naming the OSV sync; inventory and plan unchanged |

Each run is also held to `budgets` (wall seconds, peak resident memory; peak memory
is read from `/proc/<pid>/status` VmHWM on Linux, `PeakWorkingSet64` on Windows).

The goldens are a projection of the plan (`normalizePlan` in
`scripts/ci/engine-e2e-lib.mjs`): what it tells the user, without timestamps, run ids,
absolute paths or `freshness_days`.

## Running it locally

```bash
cd src-tauri
cargo build --bin fourda-engine --features e2e-osv-mirror
cd ..
node scripts/ci/engine-e2e.mjs --engine <target dir>/debug/fourda-engine[.exe] --work <dir>
```

`--work` defaults to `~/fourda-engine-e2e`; it must not be under the OS temp folder
(the engine does not check projects there). `--keep` keeps the work directory after
a pass; it is always kept after a failure (logs, `report.json`, `data/4da.db`).
On Linux the engine needs a display: `xvfb-run -a node scripts/ci/engine-e2e.mjs ...`.

## Updating the goldens

A change that is meant to alter what the plan or the coverage record says fails
this gate until the goldens follow it:

```bash
node scripts/ci/engine-e2e.mjs --engine <...>/fourda-engine --update-goldens
git diff src-tauri/tests/e2e-engine/golden/
```

Read the diff: every changed line is a change in what users are told. Commit the
goldens in the same PR as the change and say in the PR body why each difference
is right. The inventory and findings are NOT goldens: they are checked against the
conformance truth, which changes only through `../conformance/` (its README).

When adding a case, add its id to `gate.json` and update the goldens. Keep the run
inside the job's 30-minute ceiling (the gate itself should stay under 10 minutes).
