# fourda-depengine

The pure core of 4DA's dependency engine: the rules that decide whether an
installed version is affected by an advisory and which version fixes it. It is
Stage C0 of the single-engine plan: one Rust engine, built into the desktop
app and, later, into `@4da/mcp-server` as a wasm module, so the two stop
drifting apart.

## What is in it (C0.1, C0.2, C0.3)

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
| `npm_range` | `requirement_admits` / `strip_spec`: whether an npm requirement admits a version (exact bare versions, `||`, spaced operators, hyphen and x-ranges, `npm:` aliases) | moved from `src-tauri/src/osv/fix_path.rs` (re-exported there) |
| `held` (D-4) | An npm fix target judged by what it installs: `exact_pin_version`, `is_narrow_range`, `held_specs_of`, `judged_version`, `held_candidates` (at most 10), `check_release`, `judge_target` (`Moved` / `Blocked`: NOT a clean fix / `Unchecked`: NOT CHECKED), `held_note`; the host supplies registry facts and the advisory lookup (`HeldSources`) | new, the rule `@4da/mcp-server` 6.2.5 applies (`src/live/held-dependencies.ts`); the app wires it in `src-tauri/src/osv/held_targets.rs` |
| `lockfile` (C0.3) | The lockfile readers: `read_dir(&dyn FileSource, dir)`, `read_text`, `probe`, `requirements_files`; `LockFormat`, `DepScope`, `LockedPackage`, `LockfileRead`, `LockfileOutcome` (read / failed with the reason / unsupported); npm (v1-v3 + shrinkwrap), pnpm, yarn classic + berry, bun, Cargo, poetry / uv / pdm / Pipfile / requirements (`-r` includes), Go (`go.sum` before 1.17), Gemfile, composer. Every file beyond the lockfile text comes through the host's `FileSource`; `MemFs` is one held in memory | `src-tauri/src/ace/lockfile/` (`npm`, `yarn`, `bun`, `python`, `go`, `ruby` and the pure half of `mod.rs`) |
| `lockfile::flat` / `lockfile::graph` (C0.3) | The flat `(name, version)` lists of Cargo.lock / pnpm-lock.yaml / composer.lock and pnpm's key spellings; the parent -> child edges of Cargo.lock / package-lock.json / pnpm-lock.yaml (`DependencyEdge`, `EdgeScope`, `ROOT_PARENT`) | the lockfile parsers of `src-tauri/src/ace/scanner.rs` (`ProjectScanner::parse_*` now call these) |
| `lockfile::npm_bundle` (C0.3) | `inBundle` copies: `npm_bundle_owner`, `npm_lock_entry_bundled`, `is_dependency_key`, `BUNDLED_SUFFIX`, `is_bundled` | `src-tauri/src/osv/fix_path.rs` (re-exported there) |
| `lockfile::dep_scope` / `lockfile::cargo_facts` (C0.3) | AD-046 dev-vs-runtime reachability (`DevScope` for package-lock / pnpm, `CargoDevScope`); a Cargo.lock's own crates (`local_packages`, `local_only_names`, `declared_copies`) and a workspace manifest's `path` / `git` declarations | `src-tauri/src/ace/dep_scope.rs` (whole), the pure half of `src-tauri/src/ace/cargo_lock_facts.rs` |
| `api` | A provisional JSON call surface (`depengine/0`: `version_cmp`, `affected`, `fix_target`, `match_inventory`, `read_lockfiles`), exported by the wasm build | new; replaced by the versioned `depengine/1` contract later |

The app keeps its old paths through re-exports (`crate::osv::version_order`
(removed in C0.3 with its last user, the Go reader),
`crate::osv::matching::{check_version_affected, parse_version}`,
`crate::osv::fix_target::{walk_target, fix_for_version, upgrade_type, ...}`,
`crate::osv::types::{Affected, PackageRef, MatchedDependency}`,
`crate::osv::fix_target::{line_targets, LineTarget, describe_lines}`,
`crate::ace::lockfile::{LockFormat, DepScope, LockedPackage, LockfileRead, LockfileOutcome, normalize_descriptor}`,
`crate::ace::scanner::{DependencyEdge, EdgeScope, ROOT_PARENT}`,
`crate::ace::cargo_lock_facts::{local_packages, local_only_names, declared_copies, CargoDevScope}`,
`crate::osv::fix_path::{npm_bundle_owner, npm_lock_entry_bundled, BUNDLED_SUFFIX, is_bundled}`),
so no caller changed. `ProjectScanner::parse_cargo_lock` / `parse_pnpm_lock_yaml` /
`parse_composer_lock` / `parse_*_edges` keep their names as one-line calls into
`lockfile::flat` and `lockfile::graph`. `get_matched_advisories` and `exposure_for_advisory` stay in
the app as thin DB adapters: they read `osv_advisories`,
`user_dependencies` / `project_dependencies` and `dependency_instances` into
`Package` and `AdvisoryRecord` rows and call the crate.

### The host's files (`FileSource`, C0.3)

The readers never open a file. What they need beyond the lockfile text comes
through one small trait, shaped by what they actually ask:

```rust
pub trait FileSource {
    fn read(&self, path: &str) -> Result<String, String>; // Err: why ("unreadable: <err>")
    fn is_file(&self, path: &str) -> bool;                // which fixed-name lockfiles a dir holds
    fn list(&self, dir: &str) -> Option<Vec<Entry>>;      // *.csproj, *requirements*.txt, requirements/*.txt
    fn join(&self, dir: &str, name: &str) -> String { /* "/" */ }
    fn parent(&self, path: &str) -> Option<String> { /* "/" or "\" */ }
}
```

- `is_file` probes the 13 fixed names, the 11 unsupported ones and `go.sum`;
  `list` finds the files known only by pattern (`.csproj` / `.fsproj` /
  `.vbproj`, `*requirements*.txt`, `requirements/*.txt`).
- `read` returns the error text, so a lockfile the host cannot read is still
  a `Failed` outcome that says why, never a silent gap.
- `join` and `parent` belong to the host, so every path in an outcome is the
  host's own spelling (`D:\app\package-lock.json` on Windows, exactly what
  the app stored before). A companion is resolved against its lockfile:
  `go.sum` beside a pre-1.17 `go.mod`, a `-r` include beside its
  requirements file. Ordering and the include cycle guard compare paths
  component by component, as `std::path::Path` does.
- The app's host is `ace::lockfile::StdFs` (`std::fs`); `MemFs` holds the
  files in memory (tests, and the wasm `read_lockfiles` op).

## What is out

Everything that reads or writes something: OSV sync and the ZIP cache, every
HTTP call, SQLite (the DB adapters above), reachability's `cargo tree` probe
(a host hook of `match_inventory_with`), and evidence rendering. The lockfile
walk stays in the app too: which directories to read (`SKIPPED_DIR_NAMES`,
`MAX_WALK_DEPTH`, the user-scope gates in `ace_commands::dependencies`),
`StdFs`, finding the `Cargo.lock` / workspace manifest above a crate
(`ace::cargo_lock_facts::{nearest_cargo_lock, workspace_local_crate_names}`),
and the walk's report (`ace::lockfile::report`, which logs). Still in the app
and planned to move: `fix_path` / `parent_hint` with a `needs` protocol
(C0.4), and a `range_check` port (C0.5).

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
cargo test --locked                       # unit tests (the lockfile readers over MemFs included),
                                          # the version_order vectors, and tests/match_fixture.rs
                                          # (native replay of the corpus case)
node --test scripts/check-lock-drift.test.cjs
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
`src-tauri/deny.toml`. `scripts/check-lock-drift.cjs` compares the packages
both lockfiles hold (16 today). A major.minor drift that the crate's own
lockfile introduces (its version is not in the crate lockfile at the merge
base) fails the job: the crate follows the app, never leads it. Drift the
app's lockfile led only warns, so a one-sided Dependabot PR for
`/src-tauri` can merge, and the crate's PR to the same line then passes.
Patch differences are notes. The rule's both directions are tested in
`scripts/check-lock-drift.test.cjs`.

## Licence

FSL-1.1-Apache-2.0, the same as the app. Whether this crate moves to
Apache-2.0 so the wasm can ship inside the Apache-licensed `@4da/mcp-server`
is pending the operator's Decision 13 item 4 (single-engine design §3.6). Until
then the crate is internal to this repository.
