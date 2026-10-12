# fourda-depengine

The pure core of 4DA's dependency engine: the rules that decide whether an
installed version is affected by an advisory and which version fixes it. It is
Stage C0 of the single-engine plan: one Rust engine, built into the desktop
app and, later, into `@4da/mcp-server` as a wasm module, so the two stop
drifting apart.

## What is in it (C0.1)

| Module | What it does | Came from |
|---|---|---|
| `version_order` | One ordering primitive: semver, PEP 440 (local labels included), OSV's `introduced: "0"` below everything, build metadata ignored | `src-tauri/src/osv/version_order.rs` (moved whole, with its tests) |
| `range` | `check_version_affected`: is a version inside an advisory's `SEMVER` / `ECOSYSTEM` / enumerated windows, and is that confirmed or conservative; `Range`, `ENUMERATED_RANGE`, `parse_version`, `is_unknown_bound` | the pure half of `src-tauri/src/osv/matching.rs` |
| `fix_target` | `fix_for_version` (the fix on a version's own release line), `walk_target` / `clean_version` (the walk past every window with a way out, `last_affected` included), `upgrade_type` | the pure half of `src-tauri/src/osv/fix_target.rs` |
| `ecosystem` | `is_pypi`, the one ecosystem rule the above need | a subset of `src-tauri/src/ecosystem.rs`; an app test holds the two together |
| `api` | A provisional JSON call surface (`depengine/0`: `version_cmp`, `affected`, `fix_target`), exported by the wasm build | new; replaced by the versioned `depengine/1` contract later |

The app keeps its old paths through re-exports (`crate::osv::version_order`,
`crate::osv::matching::{check_version_affected, parse_version}`,
`crate::osv::fix_target::{walk_target, fix_for_version, upgrade_type, ...}`,
`crate::osv::types::{Range, ENUMERATED_RANGE}`), so no caller changed.

## What is out

Everything that reads or writes something: OSV sync and the ZIP cache, every
HTTP call, SQLite (the matcher's `get_matched_advisories` stays in the app as
the DB adapter that calls `check_version_affected`), reachability's
`cargo tree` probe, and evidence rendering. Still in the app and planned to
move: the per-project aggregation `line_targets` (it reads the app's
`MatchedAdvisory`), a pure `match_inventory` (C0.2), the lockfile readers
behind a `FileSource` (C0.3), `fix_path` / `parent_hint` with a `needs`
protocol (C0.4), and a `range_check` port (C0.5).

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
  checks a few answers. The only `unsafe` is the three wasm exports in
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

## Licence

FSL-1.1-Apache-2.0, the same as the app. Whether this crate moves to
Apache-2.0 so the wasm can ship inside the Apache-licensed `@4da/mcp-server`
is pending the operator's Decision 13 item 4 (single-engine design §3.6). Until
then the crate is internal to this repository.
