# Dependency-engine rule vectors

Stage B of the single-engine plan (AD-054 decision 4: *"the corpus is the
contract"*). The conformance corpus one directory up checks **what** each
engine finds (inventory and findings). These vectors check **which fix it
recommends and why**: version order, OSV range evaluation, the fix target,
the refresh-or-parent verdict, the command, and the MCP's `range_check`.

**The vectors, not prose, are the contract.** Every rule below is quoted from
its source and followed by the vector ids that pin it. When the prose and a
vector disagree, the vector wins until the source is corrected.

Both engines run the same files:

- **Rust app engine:** `src-tauri/src/osv/vectors_tests.rs`, in every
  `cargo test --lib` (offline, a few seconds in a debug build).
- **TypeScript MCP engine (`@4da/mcp-server`):** consumed read-only, as for
  the corpus (pinned commit, sparse checkout of `src-tauri/tests/conformance`),
  with a vitest `describe.each` over the same files. That runner is a later
  task in the MCP repository.

## Files

One JSON object per line. Every line has a stable `id`, a `rule` (the
statement it pins) and a `source` (where the expected value comes from,
specific enough to recheck).

| File | Line | Truth source |
|---|---|---|
| `version_order.jsonl` | `{eco, a, b, cmp}`, `cmp` one of `<` `=` `>` | PEP 440, SemVer 2.0.0 and the Go Modules Reference spec examples. Every line is cross-checked when written: PyPI lines with python `packaging` 25.0, the others with node `semver` 7 (Go with the `v` stripped). |
| `osv_range.jsonl` | `{record_id, eco, name, version, affected}` | OSV's own answer: `/v1/querybatch` at the exact version, recorded in `../expected/<case>.json` (records pinned 2026-10-10 in `../osv/`). `affected: true` is used only when the finding's alias group holds exactly one pinned record for that package, so OSV returned exactly that record. |
| `fix_target.jsonl` | `{eco, name, installed, record_ids[], target, clears, evidence}`, optionally `published[]`, `excluded[]`, `withheld[]` | ft-001..ft-022: the 22 cases of the 2026-10-10 fix-path oracle (the action was applied, re-resolved and re-scanned, and the target checked minimal against OSV per published version), re-checked 2026-10-12 against the pinned ids. ft-023..: OSV `/v1/querybatch` per published stable release (2026-10-12), restricted to the pinned `record_ids`. `evidence` holds the OSV ids at `installed` and at `target`. |
| `refresh_verdict.jsonl` | `{eco, name, installed, target, requirements[], verdict, via, inferred}` | The 16 transitive oracle cases (the refresh was applied: it reached the target, or it did not move the copy), each with its parents' requirements read from the case lockfile (`package-lock.json` `requires`, `yarn.lock` `dependencies`) or the parent's published manifest (npm registry for pnpm, crates.io for Cargo). Each case also appears with `requirements: []` to pin the semver fallback. The rest apply AD-055 ¶1 by hand. |
| `fix_command.jsonl` | `{tool, eco, pkg, installed, target, workspace_root, command}`; `kind: "direct"` lines add `declared` (npm family), `manifest` (Cargo), `go_mod` (Go) or `requirements` (PyPI), optionally `member: true` (the manifest sits one workspace level below the lockfile), and `instruction` | The command text of AD-055 ¶4 for a transitive copy's refresh. Where the oracle applied that tool's refresh, the `source` names the case. `kind: "direct"` lines pin the fix for a DECLARED dependency, read from the project's own manifest, that keeps how it is declared (audit E8): `command` when one command does it, `instruction` for a manifest edit, both null when nothing can be named with certainty. Each line's `source` says whether it was run against its tool (and which version) or is a rule with no tool run. |
| `range_check.jsonl` | `{eco, name, constraint, from, releases[], record_ids[], expected}` | `@4da/mcp-server` 6.2.0 recorded fixtures (`src/__tests__/fixtures/dependency-check/recorded/`, origin/main `bae421b`): OSV's querybatch answer per release of ws, Jinja2 and golang.org/x/net. Constraint membership: node `semver` (npm, Go without `v`; `v0` is the v0 line), python `packaging` `SpecifierSet` (PyPI). |
| `parent_release.jsonl` | `{eco, tool, parent{...}, rows[], expect{parent_target, rows[]}}` (below) | pr-001..pr-008: registry.npmjs.org abbreviated documents and OSV.dev `/v1/query`, read 2026-10-18 (generated once, then the rule applied by hand); OWASP/NodeGoat's `package-lock.json`; npm 10.9.2 for the install-keeps-the-copy case. pr-001..pr-004 and pr-006 agree with `@4da/mcp-server` fix/planner-advice-625 `1840dab` run on the same projects. |
| `holding.jsonl` | `{eco, name, advisory_id, version, holding}` | npm's security holding placeholders (`0.0.<n>-security`; a publisher's `1.0.0-security` prerelease is not one: ho-006): the registry's own description ("security holding package") and the advisories that flag the name. `holding: true` means no advisory applies to that release (audit L4 decision, 2026-10-16). |

Field conventions:

- `verdict` is `refresh` (a lockfile refresh reaches the target),
  `parent_must_move`, or `unknown`. `inferred` is true when no requirement
  was read and the verdict comes from semver compatibility; `unknown` always
  has `inferred: true`. `via` lists the parents whose requirement was read
  and excludes the target, in input order.
- `requirements[].bundled: true` marks a copy shipped inside its parent's tarball (package-lock `inBundle`): `text` is the exact version shipped. The optional top-level `override` says whether an override (npm `overrides`, pnpm `pnpm.overrides`, yarn `resolutions`) may be offered for the copy; lines without it do not check it.
- `requirements[].dialect` is `npm` (a bare version is an exact pin, `||`
  unions, spaces AND) or `cargo` (a bare version is a caret, commas AND).
- A `fix_target` `target` is compared as a version (build metadata ignored).
  `clears: false` means the target clears only the advisories that have a
  fix. `target: null` means no advisory on the copy has one, or (with
  `excluded`) every release that has one was withdrawn.
- Optional `fix_target` registry fields (FX-H5, 2026-10-16). Lines without
  them run on the advisory data alone, as before:
  - `published[]`: the registry's releases above `installed`, as the
    registry listed them on the date in `source`. They join the releases the
    advisory data names, so the walk can step to a release no advisory
    mentions.
  - `excluded[]`: releases the registry withdrew. crates.io and PyPI:
    `yanked`. npm: `deprecated`, but only when a later, non-deprecated,
    stable release exists on the same caret line (same major; same minor for
    `0.x`), and never when the package's `latest` is deprecated; a
    whole-major "please upgrade" notice is not a withdrawal. The line holds
    the app's rule applied to the live registry on the `source` date. No
    target may be one; the walk steps over them as it steps over
    pre-releases.
  - `withheld[]` (expected, checked only when present): the withdrawn
    releases the walk would otherwise have named, in the order met.
- `parent_release` lines (audit H1) hold one parent and the transitive rows
  it blocks, all read from the live registry and OSV on the `source` date
  and cut to the releases the rule reads:
  - `parent`: `name`, `installed`, `declared` (the spec the project's
    manifest declares it with; `null` when `lever` is `unknown`), `lever`
    (`direct`: the project declares the parent; `unknown`: no declared
    dependency can be tied to it), `latest`, `deprecated` (the `latest`
    release is deprecated), `releases[]` (its installable stable releases
    from `installed` up), `advisories[]` and `own_target` (the target of the
    parent's own plan row, from its own advisories; `null` when it has none).
  - `rows[]`: `pkg`, `installed`, `target`, `releases[]` (the package's
    installable stable releases), `advisories[]`, and `requires`
    (parent release -> its requirement on `pkg`: a string, `null` when that
    release does not depend on it, or `{"peer": "..."}`).
  - `advisories[]`: `{id, ranges: [{introduced, fixed | last_affected}]}`,
    one window per range (OSV's events, split).
  - `expect.parent_target`: the parent's own row after the raise (`null`
    without an own row). `expect.rows[]`, in row order: `outcome`
    (`release`, `deprecated`, `latest`, `none_admits`, `unread`), `smallest`
    (the smallest admitting release), `to` (the release named, lifted to
    `parent_target` when that still admits a clean copy), `breaking`,
    `requirement`, `peer`, `resolves_to` (what a fresh resolve of
    `requirement` picks), `command` (the declared operator kept; `null` for
    an unknown lever) and `refresh` (the command after the install, when
    the new requirement still admits `installed`).
- `parent_release` lines with `mode: "graph"` hold a whole installed graph
  instead: `nodes[]` (`[name, version, declared]`), `edges[]` (`[parent,
  parent version, child, the parent's requirement as written]`),
  `advisories[]` (`{id, package, ranges}`) and `registry` (per package, its
  RAW npm abbreviated document cut to the releases the rule reads: each
  release's `deprecated` notice and its requirements on the graph's
  packages, plus `dist-tags.latest`; nothing pre-filtered, so each engine
  applies its own withdrawn-release rule). `expect.rows[]` gives a declared
  package's own row (`own_target`, from its own advisories) and the release
  it rises `to`; `expect.copies[]` gives each vulnerable copy's `target` and,
  per parent `links[]`, the `release` named (`{to, smallest, floor, brings}`,
  or `none_admits` / `deprecated` / `latest` / `unread` / `undecided` /
  `malicious`), the declared `levers` it arrives through with theirs, and
  `lever_unknown`. `smallest` is always the package's OWN smallest release
  that does the job, set only when `to` was lifted above it to agree with the
  package's own row; on a lever, `floor` is the smallest parent release that
  admits the copy and `brings` the parent release `to` brings in. Commands
  are not part of graph lines.
- One predicate decides whether a release can be named, everywhere (review
  of #981, B2): a malicious-package advisory (`MAL-` id or alias) that
  affects it rules it out, and it is never "the newest"; an advisory with a
  published fix (it does not affect the newest non-malicious release) rules
  it out; an advisory no release fixes does not, and the release named says
  it remains affected by it. An advisory whose ranges cannot be read makes
  the decision `undecided`: no release and no command.
- `range_check` `releases` are the stable releases that have a recorded OSV
  answer. `record_ids` are the pinned records in `../osv/`; they are the same
  set as the MCP's recorded `/v1/query` answer for each package. A release is
  safe when OSV listed no advisory for it. `smallest_safe_upgrade` is the
  smallest safe release in range above `from`, and `null` without `from`.

## Rules and the vectors that pin them

### Version order

`osv/version_order.rs`: *"The rule for a pair of versions: a PyPI version is
read ONLY as PEP 440 (`OrderedVersion::parse_in`), so a pair with a PyPI side
orders by PEP 440. Otherwise compare as semver when BOTH parse as semver (npm,
crates.io, Go), else as PEP 440 when both parse as PEP 440, else the pair is
undecidable and the caller stays conservative."* It also says: *"Build metadata decided order.
`semver::Version`'s `Ord` compares build metadata as a tiebreaker; OSV (and
Go's `+incompatible`) treat it as no part of the precedence."* And: *"PyPI is
PEP 440, not semver. `1.0.post1`, `2.0rc1`, `1.2.3.4` and `1!2.0` failed the
semver parse."*

6.2.0 CHANGELOG: *"PyPI ... ordered by PEP 440 (`2.0`, `3.0.0rc1` and
`1.0.post1` are releases, not unreadable semver)"*; *"Go ... `+incompatible`
releases matched (`v2.0.0` finds `v2.0.0+incompatible`), pseudo-versions
dated and judged"*.

- PEP 440's example ordering list, pairwise: vo-001..vo-019
- PEP 440 epochs (`1!2.0`): vo-020, vo-021, vo-022
- Zero padding and extra release segments: vo-023..vo-027, vo-046
- PEP 440 normalisation (case, spellings, implicit post/dev numbers, `v`): vo-028..vo-033, vo-041
- Pre, post and dev phases (`rc`, `.post1`, `.dev`): vo-034..vo-040, vo-045
- A PyPI version that is also valid semver must still order by PEP 440: vo-042 (`1.0.0-1` is a post release), vo-043 (`-post1`), vo-044 (`a10 > a9`)
- SemVer §11 precedence list, for npm and crates.io: vo-047..vo-060
- Packagist and RubyGems, including two-part OSV bounds (readiness audit H2: the TS engine could not order `6.4`): vo-086..vo-091. Truth: composer/semver and PHP `version_compare`, and Gem::Version. The Packagist pair `6.4.0` / `6.4` is deliberately unpinned, because composer/semver's normaliser calls them equal and its comparator does not.
- SemVer numeric order, identifiers and pre-releases: vo-061..vo-066, vo-071, vo-072
- Build metadata is not precedence (SemVer §10): vo-067..vo-070
- Go: `v` prefix, pseudo-versions, `+incompatible`, pre-releases: vo-073..vo-085

### OSV range evaluation

The OSV schema (ossf.github.io/osv-schema): `introduced` is inclusive and
`fixed` exclusive. `last_affected` is an inclusive upper bound.
`introduced: "0"` is below every version. A version is affected when it falls
in any window of any range, or is listed in `versions`. The truth is OSV's
own evaluation of the pinned record at that exact version.

- `versions` lists: or-001..or-005
- `last_affected` (inclusive): or-006..or-013
- On a bound (introduced or fixed exactly): or-014..or-021
- Several windows or ranges: or-022..or-029
- Go pseudo-versions: or-030..or-033
- Go `+incompatible`: or-034..or-037
- PyPI versions that are not `X.Y.Z` as the lockfile spells them: or-038, or-039
- npm pre-releases: or-040, or-041
- A package named by several `affected` entries: or-042..or-049
- `introduced: "0"`: or-050..or-057
- Plain `introduced <= v < fixed`: or-058..or-067

`version` is the version as the lockfile spelled it, which is what OSV was
queried with (`3.3.0`, `v3.0.3`). The corpus inventory key is the normalised
form.

### Fix target — AD-055 ¶5

> **Targets were already right and stay one function.**
> `osv::fix_target::clean_version` walks every known advisory of the package
> to the smallest version none affects; on the oracle it printed openssl
> 0.10.80 (GHSA-phqj-4mhp-q6mq affects 0.10.50..<0.10.80), where the
> TypeScript engine printed 0.10.79. A `last_affected` window with no later
> fix declines a clean claim (braces: "clears the advisories that have a
> fix").

- The 22 oracle cases: ft-001..ft-022 (openssl 0.10.80: ft-006; braces `last_affected`: ft-004)
- OSV per published release, all four registries: ft-023..ft-061, ft-062, ft-063, ft-067, ft-068
- Packagist and RubyGems same-line fixes below a two-part bound (readiness audit H2): ft-065 (symfony/process v5.1.0 -> v5.4.51), ft-066 (rack 2.0.6 -> 2.2.23 on the pinned records)
- No clean version, so the target clears only the fixed advisories (`clears: false`): ft-004, ft-029, ft-041, ft-042, ft-055, ft-060, ft-061
- A `last_affected` window that later releases leave (the walk steps past the bound and keeps checking every advisory): ft-043 (pyjwt), ft-052 (mistune)
- A release the registry withdrew is never the target (FX-H5): npm deprecated, ft-007 (lodash 4.18.0, "Bad release", so 4.18.1) and ft-063 (brace-expansion 3.0.3, so 3.0.4); crates.io yanked, ft-062 (h2 0.3.17, so 0.3.18); every clearing release withdrawn, so no target and `withheld` names it, ft-064 (by hand)
- A whole-major npm deprecation notice is not a withdrawal, so the target stays on the installed line (review of #976): ft-067 (glob 10.4.5 -> 10.5.0, not 12.0.0), ft-068 (tar 6.1.11 -> 6.2.1, while tar 7.0.0..7.5.7 are excluded because 7.5.8 is clean)

### Refresh verdict — AD-055 ¶1, ¶2, ¶3

> 1. **One rule.** `osv::fix_path::refresh_verdict(installed, target,
>    requirements)` decides, for a copy no project declares, whether a
>    lockfile refresh reaches the target: any requirement read for a parent
>    that excludes the target sends the fix to that parent; every requirement
>    admitting it makes a refresh enough; nothing readable falls back to
>    semver compatibility (caret: same major ≥ 1.0, same minor below, any bump
>    at `0.0.z`), and every rendering says when it was inferred.
> 2. **Requirements are read before they are inferred.** The parent's
>    requirement comes from the `package-lock.json` edge (npm records
>    requirements; an exact spec there is a pin, not a resolution), else from
>    the parent's installed manifest ...
> 3. **A copy any of whose parents pins it is not "refresh is enough".**
>    traefik/webui's js-yaml 3.13.1 has three parents on `^3.13` and mocha on
>    exactly `3.13.1`; the blocking link is listed first, its copy gets no
>    command, and the Brief walks that parent's chain.

- The 16 transitive oracle cases with their parents' requirements (odd ids): rv-001..rv-031
- The same cases with nothing readable, the semver fallback (even ids): rv-002..rv-032
- The js-yaml / mocha pin (¶3): rv-017, rv-043. Its inferred twin rv-018 says `refresh`, which shows why ¶2 reads requirements first.
- npm requirement syntax (caret, tilde, `||`, hyphen, x-range, `*`, alias, operator spacing): rv-033..rv-042, rv-044..rv-047
- Any exclusion decides even with unreadable specs: rv-048. All admitting: rv-051
- Cargo requirement syntax (bare = caret, `=`, `~`, comma AND): rv-052..rv-059
- The semver fallback, including the 0.0.z caret: rv-049, rv-050, rv-060..rv-064 (0.0.z: rv-046, rv-057, rv-063, rv-064)
- Nothing readable at all, so no verdict: rv-065
- A bundled copy (package-lock `inBundle`, requirement `bundled: true`) is pinned by its owner: the parent must move and no override is offered (`override: false`): rv-066, rv-067. An ordinary exact pin also needs its parent to move but does take an override: rv-068 (audit E8)

### Command — AD-055 ¶4

> **The command is named for tools it can stand behind:** `npm update <pkg>`,
> `pnpm update <pkg> --depth Infinity` (`-r` at a pnpm workspace root),
> `yarn up -R <pkg>` (berry), `cargo update -p <pkg>@<installed> --precise
> <target>` ... Yarn 1 has no command that moves one transitive copy in
> place, so none is named.

- npm: fc-001..fc-004
- pnpm, and `-r` at a workspace root: fc-005..fc-008
- yarn berry: fc-009, fc-010
- yarn 1, no command: fc-011, fc-012
- Cargo: fc-013..fc-020
- Tools no command is named for (pip, poetry, uv, pipenv, go, bundler, composer): fc-021..fc-027
- A direct dependency (`kind: "direct"`, audit E8), with the declared operator kept:
  - npm: `npm install x@v --save-exact` for an exact pin, or `npm install "x@^v"` / `"x@~v"` (quoted, because `^` is cmd.exe's escape character): fc-028..fc-031;
  - pnpm: `pnpm up`: fc-032, fc-033;
  - yarn 1 and berry: `yarn upgrade` / `yarn up` with the operator: fc-034, fc-035;
  - no command for a spec whose operator cannot be carried: fc-036;
  - no command when the declared spec does not admit the installed copy: fc-046 (npm), fc-047 (Cargo);
  - no command for a yarn workspace member, because `yarn upgrade` / `yarn up` rewrite every workspace: fc-053.
- A direct Cargo dependency: `cargo add`, with `--dev` / `--build` / `--target` / `--rename` from the declaring table and the operator kept (fc-037..fc-042; `--target` double-quoted, because cmd.exe passes single quotes through: fc-040); an instruction to edit the workspace root for a workspace-inherited crate, where `cargo add` replaced the inheritance with a local pin (fc-043); nothing for a git, path or alternate-registry source (fc-048).
- A direct dependency whose declared range already admits the target gets the tool's in-range lockfile refresh, never an install that rewrites the declaration (`refresh: true`; each run 2026-10-18 with package.json unchanged): `npm update x` (fc-054), `pnpm update x --no-save` (fc-055; plain `pnpm update` rewrote the range), `yarn upgrade x` (fc-056), `yarn up -R x` (fc-057), `cargo update -p x@v --precise t` (fc-058), an npm and a pnpm workspace member, run in the member's directory (fc-060, fc-061). Only a range bounded below the next major (`^`, `~`) is refreshed, because a refresh installs the newest release in range; a `>=` range names nothing (fc-059).
- A direct Go requirement: `go get mod@vX`; none for an indirect one or one under a `replace`: fc-044, fc-045, fc-049.
- A direct PyPI `==` pin: an instruction that replaces only the version token and keeps extras and the environment marker (fc-050); nothing for a hashed pin (fc-051) or pip-compile output (fc-052).

### Constraint checks — `@4da/mcp-server` 6.2.0 CHANGELOG

> **Constraints.** `to` may be the constraint the agent was given: `"7.x"`,
> `"^1.20"`, `"=0.17"`, `"~=3.1"`, `"3.1.*"`, `">=2,<3"`, `"v1"`. The item
> then returns `range_check` (`smallest_safe_in_range`,
> `newest_safe_in_range`, `safe_versions`, `smallest_safe_upgrade` above
> `from`), from every advisory OSV.dev lists for the package evaluated
> against each release the constraint admits ... A test holds the local range
> evaluation to OSV.dev's own per-version answers on recorded data (ws,
> Jinja2, golang.org/x/net).

- ws (npm), including "pin ws 7.x" -> 7.5.11 / 7.5.13 and the line with no safe release: rc-001..rc-012
- Jinja2 (PyPI), including `3.1.*` -> 3.1.6 with 6 vulnerable: rc-013..rc-022
- golang.org/x/net (Go), including `v0` -> v0.60.0 / v0.61.0: rc-023..rc-028

The release-age hold and withdrawn or deprecated releases are not part of
these `range_check` vectors. They depend on the clock and on registry
metadata. (Withdrawn releases as fix targets are pinned in `fix_target.jsonl`
with the registry state written into the line, above.)

### Parent release — readiness audit H1

`@4da/mcp-server` fix/planner-advice-625 (`1840dab`,
`src/tools/upgrade-planner-fixpaths.ts`): *"the smallest parent release whose
own declared requirement admits a clean version, read from the registry, and
that is itself clear of the parent's own advisories; then the dependency the
project DECLARES that moves it ... A parent the walk cannot tie to a
declaration names no lever and no command ... When the new requirement still
admits the installed copy, an install keeps that copy (npm 10.9.2, checked
2026-10-17: express 4.21.2 -> 4.22.0 kept path-to-regexp 0.1.12 under
`~0.1.12`), so the step names the refresh that follows."* And
`reconcileDirectTargets`: *"One dependency, one command ... a direct row and
the parent upgrades that name it as their lever agree on its release."*

- NodeGoat (express 4.16.4, path-to-regexp 0.1.7, qs 6.5.2): the express row
  rises from 4.20.0 to 4.22.3 and both transitive rows name 4.22.3, with the
  declared `^` kept: pr-001
- The smallest admitting release, clear of the parent's own advisories, and
  the parent's own row raised to it (express 4.22.0, exact pin kept): pr-002
- The refresh after an install that keeps the vulnerable copy: pr-003
- A move across a major version is flagged (mongodb-core 2.1.20 -> 3.1.2): pr-004
- A parent no declared dependency can be tied to: its release, no command: pr-005
- A parent whose latest release is deprecated names no release: pr-006
- Graph mode (`mode: "graph"`, review of #981 S5), the whole pipeline from
  RAW registry documents: NodeGoat's express, path-to-regexp, qs,
  serve-static and send, one release (express 4.22.3) on every row,
  including a transitive parent's declared lever (serve-static via express)
  lifted to the lever's own row: pr-007
- A whole old major deprecated with one notice is not withdrawn (the decided
  FX-H5 rule; the engines must agree): glob 11.0.3's target stays 11.1.0,
  and @nestjs/cli 11.0.11 is the release that admits it: pr-008

## `engines_skip.json` — the shrink-only gate

```jsonc
{ "schema": 1,
  "rust": { "<vector id>": "<why this engine does not satisfy it yet>" },
  "ts":   { } }    // owned by the MCP runner; empty until it lands
```

Each engine's runner:

1. fails on a vector that fails and is not in its section;
2. fails on a skipped vector that now passes, so the entry must be removed;
3. fails when its section grows past a committed baseline (Rust:
   `RUST_SKIP_BASELINE` in `vectors_tests.rs`), and asks for the baseline to
   be lowered when the section shrinks;
4. fails on a skip entry that names no vector, or has no reason.

A skip entry is a known disagreement with a stated truth, not an opinion.
Fixing the engine removes the entry. If the truth is wrong, fix the vector
and say why in the commit.

## Rust skip list (2026-10-12)

28 entries, `RUST_SKIP_BASELINE = 28`.

| Ids | Reason |
|---|---|
| rc-001..rc-028 | Not implemented in Rust. `range_check` exists only in `@4da/mcp-server` (6.2.0 `dependency_check`). |

Fixed and removed (2026-10-12):

- **vo-013..vo-015:** `version_order::parse_pep440` now keeps the local
  label and orders it per PEP 440 (`1.0 < 1.0+abc.5 < 1.0+abc.7 < 1.0+5`).
- **vo-042..vo-044:** a PyPI version is read only as PEP 440
  (`OrderedVersion::parse_in`, and every matcher entry point now takes the
  advisory's ecosystem), so `1.0.0-1` / `1.0.0-post1` are post releases and
  `a10 > a9`. Other ecosystems order exactly as before.
- **ft-043, ft-052:** `fix_target::walk_target` leaves a `last_affected`
  window at the first release the advisory data names above the bound, and
  keeps checking every advisory until it reaches a version none with a way
  out affects. pyjwt 2.9.0 now claims 2.15.0 clean; mistune 0.8.3 moves
  from 3.3.0 (still affected by GHSA-6m44-fpc8-c3rq / PYSEC-2026-3863) to
  3.3.3.

## Regenerating

The vectors are generated once and then edited by hand; they are not
regenerated from either engine. A new vector needs a truth source of the
kinds above, and its expected value must never be copied from an engine's
output (that would make it vacuous). Network access is needed only when
writing the file (OSV, registries); the runners are offline.
