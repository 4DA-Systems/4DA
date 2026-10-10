# Dependency-engine conformance corpus

A hermetic, deterministic corpus that both 4DA dependency engines are scored
against: the Rust app engine (`src-tauri/src/osv/conformance_tests.rs`, runs in
every `cargo test --lib`) and the TypeScript MCP server
(`@4da/mcp-server`, consumer to follow — see [Consuming from the MCP
repo](#consuming-from-the-mcp-repo)). It is launch gate **G1**: precision >= 99.5 %
and recall >= 99 % on the lockfiles read, zero silent drops, >= 30 repositories,
>= 8 lockfile formats.

Nothing here touches the network at test time. Lockfiles are vendored, the OSV
advisories that name their packages are pinned, and the truth is checked in.

## Layout

```
cases/<id>/case.json        where the case came from (repo, tag, commit, licence, formats, notes)
cases/<id>/<upstream path>  vendored lockfiles + the manifests the readers consult, upstream-relative
osv/<ID>.json               pinned OSV records, byte-for-byte as served by api.osv.dev/v1/vulns/<ID>
osv-snapshot.json           when the records were fetched, and how many
expected/<id>.json          the adjudicated truth for one case
adjudications.json          every correction to the raw oracle, and every engine disagreement, with a reason
thresholds.json             the per-ecosystem ratchet the Rust engine must not fall below
```

Only lockfiles and the manifests a reader consults (`package.json`, `Cargo.toml`,
`pyproject.toml`, `Pipfile`, `go.mod`, `Gemfile`, `composer.json`) are vendored —
never source code. Text files are normalised to LF.

## The cases

35 cases: 32 from public repositories at a pinned commit, 3 synthetic.
Every repository case is permissively licensed (MIT, Apache-2.0, BSD-3-Clause,
ISC, Unlicense); `case.json` records the licence. Three cases used in the
2026-10-09 fixture audit were NOT vendored because their repositories are AGPL
or unlicensed (lencx/ChatGPT, spacedrive, proshop_mern); permissive cases cover
the same formats (Tauri app, pnpm v6, npm v1).

| case | source | licence | formats | KB | packages | findings |
|---|---|---|---|---:|---:|---:|
| `go-caddy-v2.6` | caddyserver/caddy@v2.6.0 | Apache-2.0 | go.mod-go1.18, go.sum | 148 | 126 | 76 |
| `go-gh-cli-v2.20` | cli/cli@v2.20.0 | MIT | go.mod-go1.18, go.sum | 57 | 72 | 39 |
| `go-hugo-legacy` | gohugoio/hugo@v0.80.0 | Apache-2.0 | go.mod-go1.12, go.sum, npm-v1 | 84 | 135 | 96 |
| `go-node-exporter-legacy` | prometheus/node_exporter@v1.0.1 | Apache-2.0 | go.mod-go1.14, go.sum | 52 | 56 | 67 |
| `java-petclinic-maven` | spring-projects/spring-petclinic@02babdd | Apache-2.0 | pom.xml (unsupported) | 13 | 0 | 0 |
| `js-express-boilerplate-yarn1` | hagopj13/node-express-boilerplate@v1.7.0 | MIT | yarn-v1 | 303 | 978 | 195 |
| `js-h3-pnpm9` | unjs/h3@v1.12.0 | MIT | pnpm-v9.0 | 250 | 929 | 167 |
| `js-hono-bun` | honojs/hono@v4.7.0 | MIT | yarn-v1, bun-binary (unsupported), npm-v3, yarn-berry, bun-text | 393 | 1294 | 290 |
| `js-jest-yarn-berry` | jestjs/jest@v29.0.0 | MIT | yarn-berry | 814 | 2025 | 283 |
| `js-nest-npm3` | nestjs/nest@v10.0.0 | MIT | npm-v3 | 713 | 1889 | 367 |
| `js-taxonomy-pnpm6` | shadcn-ui/taxonomy@651f984 | MIT | pnpm-v6.0 | 298 | 987 | 196 |
| `js-trpc-pnpm5` | trpc/trpc@v10.9.0 | MIT | pnpm-v5.4 | 833 | 2421 | 472 |
| `php-js-symfony-demo` | symfony/demo@v1.6.0 | MIT | composer.lock, yarn-v1 | 593 | 1076 | 265 |
| `poly-fastapi-template-poetry` | fastapi/full-stack-fastapi-template@0.6.0 | MIT | poetry.lock, npm-v2 | 541 | 507 | 174 |
| `poly-fastapi-template-uv` | fastapi/full-stack-fastapi-template@0.8.0 | MIT | uv.lock, npm-v2 | 474 | 391 | 154 |
| `poly-superset-subset` | apache/superset@2.1.0 | Apache-2.0 | requirements, npm-v2 | 488 | 599 | 78 |
| `poly-traefik-v2.6` | traefik/traefik@v2.6.0 | MIT | requirements, go.mod-go1.16, go.sum, npm-v1 | 863 | 1676 | 465 |
| `py-fastapi-requirements` | fastapi/fastapi@0.100.0 | MIT | requirements | 7 | 11 | 25 |
| `py-flask-pip-compile` | pallets/flask@2.0.0 | BSD-3-Clause | requirements | 6 | 55 | 36 |
| `py-httpbin-pipfile` | postmanlabs/httpbin@f8ec666 | ISC | Pipfile.lock | 14 | 19 | 40 |
| `py-js-ctfd` | CTFd/CTFd@3.5.0 | Apache-2.0 | requirements, yarn-v1 | 279 | 911 | 157 |
| `py-langchain-poetry-subset` | langchain-ai/langchain@v0.1.0 | MIT | requirements, poetry.lock | 187 | 139 | 104 |
| `py-poetry-1.1` | python-poetry/poetry@1.1.0 | MIT | poetry.lock | 68 | 83 | 49 |
| `py-pydantic-uv` | pydantic/pydantic@v2.10.0 | MIT | uv.lock | 349 | 110 | 50 |
| `py-requests-html-pipfile` | psf/requests-html@v0.10.0 | MIT | Pipfile.lock | 29 | 45 | 47 |
| `rb-js-rails-6.0` | rails/rails@v6.0.0 | MIT | Gemfile.lock, yarn-v1 | 262 | 1014 | 321 |
| `rs-alacritty-platform` | alacritty/alacritty@v0.10.0 | Apache-2.0 | Cargo.lock | 53 | 201 | 17 |
| `rs-bat-0.18` | sharkdp/bat@v0.18.0 | Apache-2.0 OR MIT | Cargo.lock | 36 | 146 | 26 |
| `rs-js-tauri-workspace` | tauri-apps/tauri@tauri-v2.0.0 | Apache-2.0 OR MIT | Cargo.lock, pnpm-v9.0 | 435 | 1474 | 199 |
| `rs-nushell-workspace` | nushell/nushell@0.60.0 | MIT | Cargo.lock, npm-v1 | 157 | 552 | 128 |
| `rs-pake-tauri-app` | tw93/Pake@V2.3.0 | MIT | Cargo.lock | 114 | 457 | 46 |
| `rs-ripgrep-12` | BurntSushi/ripgrep@12.0.0 | Unlicense OR MIT | Cargo.lock | 37 | 55 | 7 |
| `synth-go-replace` | synthetic | CC0-1.0 | go.mod-go1.21, go.sum | 2 | 8 | 34 |
| `synth-npm-edges` | synthetic | CC0-1.0 | npm-v3 | 4 | 9 | 14 |
| `synth-python-requirements` | synthetic | CC0-1.0 | requirements | 1 | 8 | 101 |

Vendored size: cases 8.7 MB, osv 12.8 MB, expected 1.4 MB (about 23 MB on disk, over the 15 MB target). The pinned OSV set is the full advisory history of every inventoried package name, which is what lets the corpus catch false positives on versions outside today's matches; it alone is 12.8 MB. JSON compresses well in the repository's object store.

Giant monorepos are trimmed to a representative subset, and `case.json` says
so: langchain (104 lockfiles, 22 MB -> libs/core + a docs requirements file),
superset (frontend/cypress/embedded-sdk locks omitted), jest (25 e2e fixture
locks omitted), nest (37 sample-app locks omitted), rails (dummy-app yarn locks
omitted), h3 (binary docs lock omitted).

Formats covered: npm `package-lock.json` v1/v2/v3, pnpm v5.4/v6.0/v9.0, yarn v1
and berry, bun text `bun.lock`, Cargo (workspace, Tauri framework, Tauri app,
platform-specific trees, the old v1 lock format), poetry, uv, Pipfile.lock,
requirements files (pinned, `-r` includes, hashes, extras, markers, `===`,
pip-compile output, unpinned ranges, VCS/editable lines), go.mod >= 1.17 and
< 1.17 with `replace` and pseudo-versions, Gemfile.lock, composer.lock, plus two
declared-unsupported files (binary `bun.lockb`, Maven `pom.xml`).

Deliberate edge cases: nested `node_modules` duplicates at different versions,
workspace/linked packages that share a registry name (`synth-npm-edges`'s local
`express@4.0.0`, jest's `0.0.0-use.local` members, Cargo workspace crates, a uv
editable root, rails' PATH gems), platform-specific Cargo dependencies, dev-only
subtrees, pnpm peer-suffixed keys (v5 `_peer`, v6/v9 `(peer)`), Go `replace` to a
version and to a local directory, Python extras/markers/hashes, an empty
lockfile (0 packages, must not count as a drop).

## `expected/<id>.json` (schema 1)

```jsonc
{
  "schema": 1,
  "case": "js-taxonomy-pnpm6",
  "lockfiles": [            // EVERY lockfile-like file in the case dir
    { "path": "pnpm-lock.yaml", "dir": ".", "format": "pnpm-v6.0", "ecosystem": "npm",
      "status": "supported", "truth_packages": 937 },
    { "path": "benchmarks/query-param/bun.lockb", "dir": "benchmarks/query-param",
      "format": "bun-binary", "ecosystem": "npm",
      "status": "unsupported", "reason": "binary bun lockfile ..." }
  ],
  "inventory": { "count": 937, "packages": ["npm|.|@babel/code-frame|7.21.4", "..."] },
  "findings": [
    { "ecosystem": "npm", "dir": ".", "package": "next", "version": "13.3.2-canary.2",
      "ids": ["GHSA-..."] }      // one alias group; any id in it identifies the finding
  ]
}
```

- **Lockfile-like** = `package-lock.json`, `npm-shrinkwrap.json`, `pnpm-lock.yaml`,
  `yarn.lock`, `bun.lock`, `bun.lockb`, `Cargo.lock`, `poetry.lock`, `uv.lock`,
  `Pipfile.lock`, `go.mod`, `go.sum`, `Gemfile.lock`, `composer.lock`, `pom.xml`,
  `gradle.lockfile`, any `requirements*.txt`, and any `.txt` under a
  `requirements/` directory. Every one must be declared `supported` or
  `unsupported` (with a reason) — an undeclared lockfile fails the shape test.
- **Grain** = (ecosystem, project directory relative to the case root, package,
  installed version). A lockfile's project directory (`dir`) is the
  directory that holds it, except that a `.txt` inside a `requirements/`
  directory (pip-compile layout) belongs to the project directory above it. Ecosystems use OSV names: `npm`, `crates.io`, `PyPI`, `Go`,
  `RubyGems`, `Packagist`.
- **Key normalisation** (inventory keys are stored normalised; both consumers
  apply the same rules to their own output): PyPI names PEP 503
  (lowercase, runs of `-_.` -> `-`), Go names verbatim, every other name
  lowercased; Go versions lose a leading `v`; PyPI versions are lowercased,
  lose a leading `v`, and drop trailing `.0` release segments (`2.0.0` -> `2`,
  `1.0rc1` -> `1rc1`).
- **Findings** are OSV's own evaluation (`/v1/querybatch` with the exact
  version) of every inventory entry, grouped by alias (union of each pinned
  record's `id` and `aliases`). A consumer's finding matches when its key is
  equal and its advisory id or any alias is in `ids`.
- Go `stdlib`/`toolchain` advisories are out of scope (a `go` directive is a
  language floor, not an installed toolchain).

## How the truth is built — and adjudicated

1. **Inventory oracle**: osv-scanner 2.6.0, `scan source --all-packages
   --no-resolve --no-ignore` (no deps.dev resolution: only what the lockfile
   says). Lockfiles the directory walk skips get an explicit `-L` parser;
   a lockfile osv-scanner cannot parse at all (trpc's pnpm 5.4 lock) falls back
   to the script's own minimal reader and is recorded as `extractor-fallback`.
2. **Go** is never taken from go.sum: go >= 1.17 uses the complete go.mod
   require set, older modules use `go list -m all` (Go's MVS build list)
   restricted to modules whose source hash go.sum records. `replace` applied.
3. **Mechanical corrections** (`adjudications.json` -> `truth_rules`,
   `truth_corrections`): local/workspace packages removed (osv-scanner lists
   e.g. a workspace member named `express` as a registry package), pnpm peer
   suffixes stripped, unpinned requirements excluded, declared-unsupported
   files excluded.
4. **Manual overrides** (`truth_overrides`): hand-reviewed additions/removals,
   each with `reason_code`, `reason` and the lockfile it rests on. Preserved
   across regeneration.
5. **Engine disagreements** (`engine_disagreements`): every inventory and
   finding difference between the Rust engine and the truth at generation
   time, grouped by verdict, e.g. `engine-gap` (lockfile not read),
   `graph-only module` (go.sum holds only its go.mod hash),
   `local/workspace package read as a registry install`,
   `gem platform suffix kept in the version`. Verdicts are mechanical (raw lockfile text, the
   lockfile's own local-source markers, OSV's evaluator); none may stay
   `UNRESOLVED`. This section is a snapshot: regenerate it with the engine dump
   when the engine changes.

## The ratchet (`thresholds.json`)

Per OSV ecosystem: `findings_precision`, `findings_recall`,
`inventory_precision`, `inventory_recall`, plus `max_silent_drops` (supported
lockfiles with truth packages from which the engine read nothing). The values
are what the engine measured when the corpus landed; the test fails if any
falls. When a fix moves a number up, the test prints `ratchet can rise` — raise
the file in the same PR. Since W1-F (2026-10-10) every ecosystem measures
1.0 on all four and `engine_disagreements` is empty, so any new disagreement
fails the ratchet. Precision with no claims and recall with nothing to
find are 1.0. Only version-confirmed matches are scored; the run prints the
count of unconfirmed ones.

## Running

```bash
cd src-tauri
cargo test --lib osv::conformance_tests -- --nocapture   # report + ratchet, ~10 s alone (debug)
FOURDA_CONFORMANCE_DUMP=/tmp/dump cargo test --lib osv::conformance_tests
                                     # also writes <case>.json (engine inventory + findings) and diff.tsv
```

Under nextest (CI) the test runs in its own process (~10 s); under plain `cargo test` it shares the CPU with the rest of the suite and takes longer.
The secret scanners skip `osv/` (pre-commit `STAGED_EXCLUDE`, pre-push pathspec): the unmodified advisory texts quote example credentials from the vulnerable projects.

The Rust harness reads each case through the lockfile walk's own API:
`ace::lockfile::walk_dirs` (directory selection: skip list + depth, without
the user-scope gates) and `ace::lockfile::read_dir` (every reader). It then
stores the reads merged per ecosystem, as `process_lockfile_dir` does. It
leaves out the parts that cannot change a match: direct/dev labels, edges, the
`cargo tree` probe and prunes.

## Regenerating

```bash
node scripts/conformance-corpus.mjs all --osv-scanner <path/to/osv-scanner> \
     --cache <scratch dir> --engine-dump <dir written by the harness>
```

Steps (each also runs alone, `--only id,id` limits cases): `fetch` (sparse,
blob-filtered clone at the pinned commit), `vendor`, `scan` (osv-scanner + Go),
`pin` (every OSV record naming a package in the truth or engine inventory),
`truth` (expected files + adjudications.json). Needs network, osv-scanner >= 2.6
and a Go toolchain. Then re-run the Rust test and update `thresholds.json` to
the measured values. To add a case: write `cases/<id>/case.json` (repository
cases: `repo`, `ref`, `commit`, `licence`, `include`, `formats`, `notes`;
`include` accepts `@cargo-manifests` and `@workspaces`), run the steps with
`--only <id>`, review its `engine_disagreements`, commit.

## Consuming from the MCP repo

The TypeScript engine consumes this directory read-only (vendored by a pinned
git commit or fetched as a tarball in CI — never edited there):

1. For each `cases/<id>/`, run the engine's lockfile discovery on that
   directory with networking disabled and an advisory source that serves ONLY
   `osv/*.json` (load every record; key by affected `package.ecosystem` +
   `package.name`).
2. Map each reported package to `ecosystem|dir|name|version` (dir relative to
   the case root, `.` for the root) with the normalisation rules above; same
   for each version-confirmed vulnerability plus its advisory id and aliases.
3. Score per ecosystem exactly as the Rust harness does: inventory and findings
   precision/recall, plus silent drops (a `supported` lockfile with
   `truth_packages > 0` and no reported package in its `dir|ecosystem`), and
   assert against a `thresholds.json` of the MCP repo's own (the two engines
   ratchet independently).
4. Adjudications for TS-engine disagreements go in a `engine_disagreements`
   section of the MCP repo's own copy of the report, never by editing truth
   here; a truth error found from either engine is fixed HERE, via
   `truth_overrides`, and both consumers pick it up.

## Licences and attribution

- Vendored lockfiles/manifests: copyright their upstream authors, used under
  the licence recorded in each `case.json` (all permissive). Synthetic cases:
  CC0-1.0.
- OSV records (`osv/`): from [OSV.dev](https://osv.dev), redistributed under
  their sources' terms — GitHub Advisory Database (GHSA) CC-BY-4.0, PyPA
  advisory database (PYSEC) CC-BY-4.0, Go vulnerability database (GO)
  CC-BY-4.0, RustSec (RUSTSEC) CC0-1.0, OpenSSF malicious-packages (MAL)
  Apache-2.0. Records are unmodified.
