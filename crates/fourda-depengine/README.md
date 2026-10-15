# fourda-depengine

The pure core of 4DA's dependency engine: the rules that decide whether an
installed version is affected by an advisory and which version fixes it. It is
Stage C0 of the single-engine plan: one Rust engine, built into the desktop
app and, later, into `@4da/mcp-server` as a wasm module, so the two stop
drifting apart.

## What is in it (C0.1, C0.2)

| Module | What it does | Came from |
|---|---|---|
| `version_order` | One ordering primitive: semver, PEP 440 (local labels included), OSV's `introduced: "0"` below everything, build metadata ignored | `src-tauri/src/osv/version_order.rs` (moved whole, with its tests) |
| `range` | `check_version_affected`: is a version inside an advisory's `SEMVER` / `ECOSYSTEM` / enumerated windows, and is that confirmed or conservative; `Range`, `ENUMERATED_RANGE`, `parse_version`, `is_unknown_bound` | the pure half of `src-tauri/src/osv/matching.rs` |
| `fix_target` | `fix_for_version` (the fix on a version's own release line), `walk_target` / `clean_version` (the walk past every window with a way out, `last_affected` included), `upgrade_type` | the pure half of `src-tauri/src/osv/fix_target.rs` |
| `ecosystem` | `osv_name` / `normalize` (every alias the app stores to its OSV ecosystem) and `is_pypi` | a subset of `src-tauri/src/ecosystem.rs`; an app test holds the alias tables together |
| `osv` (C0.2) | `OsvRecord`, `Affected`, `PackageRef`, `merge_affected` (one stored row per package, branches unioned, `versions` as an enumerated range), `stored_records` / `AdvisoryRecord` (what the mirror stores) | the pure half of `src-tauri/src/osv/sync.rs` and `types.rs` |
| `matching` (C0.2) | `match_inventory(&[Package], &[AdvisoryRecord]) -> Vec<Finding>`: which installed copies each advisory affects, confirmed or conservative, each copy's line fix and walk target; `match_inventory_with` adds a host hook (the app's AD-051 reachability filter); `package_key`, `pep503_name`, `MatchedDependency` | the rules of `src-tauri/src/osv/matching.rs` |
| `exposure` (C0.2) | `exposure_for_record`: the same per-copy check for one advisory (the scorer's verdict) | the pure half of `src-tauri/src/osv/advisory_match.rs` |
| `lines` (C0.2) | `line_targets`, `LineTarget`, `distinct_targets`, `describe_lines`: per-project, per-release-line upgrade targets | `src-tauri/src/osv/fix_target.rs` |
| `api` | A provisional JSON call surface (`depengine/0`: `version_cmp`, `affected`, `fix_target`, `match_inventory`), exported by the wasm build | new; replaced by the versioned `depengine/1` contract later |

The app keeps its old paths through re-exports (`crate::osv::version_order`,
`crate::osv::matching::{check_version_affected, parse_version}`,
`crate::osv::fix_target::{walk_target, fix_for_version, upgrade_type, ...}`,
`crate::osv::types::{Affected, PackageRef, MatchedDependency}`,
`crate::osv::fix_target::{line_targets, LineTarget, describe_lines}`), so no
caller changed. `get_matched_advisories` and `exposure_for_advisory` stay in
the app as thin DB adapters: they read `osv_advisories`,
`user_dependencies` / `project_dependencies` and `dependency_instances` into
`Package` and `AdvisoryRecord` rows and call the crate.

## What is out

Everything that reads or writes something: OSV sync and the ZIP cache, every
HTTP call, SQLite (the DB adapters above), reachability's `cargo tree` probe
(a host hook of `match_inventory_with`), and evidence rendering. Still in the
app and planned to move: the lockfile readers behind a `FileSource` (C0.3),
`fix_path` / `parent_hint` with a `needs` protocol (C0.4), and a
`range_check` port (C0.5).

## Purity rules

- No `std::fs`, no network, no database, no clock, no environment, no
  threads. Inputs are strings and structs; outputs are values. Logging becomes
  returned diagnostics.
- Dependencies: `serde`, `serde_json`, `semver`, `regex` (and, if needed,
  `chrono` with `default-features = false`). Never `tokio`, `reqwest`,
  `rusqlite` or `tracing`.
- The proof is mechanical: CI builds the crate for `wasm32-unknown-unknown`
  and `scripts/check-wasm.cjs` fails on **any** import (no WASI, no host
  functions), then instantiates the module with an empty import object and
  checks a few answers. It then replays one real conformance case
  (`tests/fixtures/match_inventory_case.json`: the inventory the app's
  lockfile readers produce for `py-fastapi-requirements`, the pinned OSV
  records naming its packages, and the native answer) through the module and
  requires wasm == native. `tests/match_fixture.rs` replays it natively, and
  the app's conformance harness rewrites it with `FOURDA_DEPENGINE_FIXTURE=1`.
  The only `unsafe` is the three wasm exports in
  `src/wasm.rs`, compiled on wasm32 only.
- Tests may read files (the rule vectors in
  `src-tauri/tests/conformance/vectors/`); the library may not.

## Building and testing

```bash
cd crates/fourda-depengine
cargo test --locked                       # unit tests + the version_order vectors
cargo clippy --locked -- -D warnings
CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_RELEASE_LTO=true \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_STRIP=true \
  cargo rustc --lib --release --locked --target wasm32-unknown-unknown --crate-type cdylib
node scripts/check-wasm.cjs target/wasm32-unknown-unknown/release/fourda_depengine.wasm
```

The crate is a plain `rlib` in its manifest, so the app never links a
`cdylib`; the wasm build asks for one on the command line. The release-profile
settings come from the environment because a dependency's own `[profile]`
would be ignored inside the app build anyway.

The authoritative gates still run in the app: `osv::vectors_tests` (all six
vector files, shrink-only skip list) and `osv::conformance_tests` (the
corpus, 1.0 on every ecosystem) both call this crate's functions.

## Supply chain

The crate keeps its own `Cargo.lock` for the standalone build and the wasm
module; inside the app it resolves through `src-tauri/Cargo.lock`. That
lockfile is covered on its own: Dependabot (`/crates/fourda-depengine`), and
the depengine CI job runs `cargo audit` and `cargo deny check` against
`src-tauri/deny.toml`. `scripts/check-lock-drift.cjs` fails the job when a
package both lockfiles hold differs in major.minor (a patch difference is
reported, not failed), so the wasm and the app cannot run a different minor
of `semver`, `regex` or `serde_json`.

## Licence

FSL-1.1-Apache-2.0, the same as the app. Whether this crate moves to
Apache-2.0 so the wasm can ship inside the Apache-licensed `@4da/mcp-server`
is pending the operator's Decision 13 item 4 (single-engine design §3.6). Until
then the crate is internal to this repository.
