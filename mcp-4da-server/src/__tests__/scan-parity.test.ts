// SPDX-License-Identifier: Apache-2.0
/**
 * Lockfile reading at parity with osv-scanner (2026-10-02).
 *
 * Measured against osv-scanner, npm audit, cargo audit and pip-audit on 12
 * projects, the matching layer was exact and every loss happened while
 * reading lockfiles: one version kept per package name, truncated Go
 * pseudo-versions, requirements.txt extras and inline comments, unread Poetry
 * and Cargo-workspace projects, semver-only version ordering for PyPI, and
 * case-sensitive Python names. Each case below is one of those losses, pinned
 * offline.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { resolveVersionSource, readPackageLock, parseCargoLockPackages } from "../live/lockfile-parsers.js";
import { parseGoMod, parseRequirements, readTomlPackageLock, selectedFromGoSum, compareGoVersions } from "../live/lockfile-parsers-pygo.js";
import { readPnpmLock, readYarnLock } from "../live/js-lockfile-readers.js";
import { InstanceSet } from "../live/lockfile-types.js";
import { resolveAuditVersions } from "../live/version-resolver.js";
import { compareVersions, isComparable, parsePep440 } from "../live/version-compare.js";
import { extractFixedVersion } from "../live/osv-scanner.js";
import { cargoWorkspaceDeps, parsePyprojectDependencies } from "../project-manifests.js";
import { scanProjectTree, treeResolutionGroups } from "../project-tree.js";
import type { OsvVulnerability } from "../live/types.js";

let dir: string;
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), "4da-parity-"));
});
afterEach(() => {
  fs.rmSync(dir, { recursive: true, force: true });
});
const write = (rel: string, content: string) => {
  const file = path.join(dir, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
};

describe("npm package-lock: every installed copy, the hoisted one as the direct version", () => {
  const lock = {
    packages: {
      "": { dependencies: { minimist: "^1.2.0" } },
      "node_modules/minimist": { version: "1.2.0" },
      "node_modules/mkdirp": { version: "0.5.1", dev: true },
      "node_modules/mkdirp/node_modules/minimist": { version: "0.0.8", dev: true },
      "node_modules/optional-thing": { version: "2.0.0", devOptional: true },
      "packages/app": { version: "1.0.0" },
      "node_modules/app": { link: true },
    },
  };

  it("keeps the nested minimist 0.0.8 beside the direct 1.2.0", () => {
    const { versions, instances } = readPackageLock(lock);
    expect(versions.get("minimist")).toBe("1.2.0");
    const minimist = instances.toArray().filter((i) => i.name === "minimist").map((i) => i.version).sort();
    expect(minimist).toEqual(["0.0.8", "1.2.0"]);
  });

  it("reads npm's dev flags: dev-only copies are dev, devOptional ones runtime", () => {
    const byKey = new Map(readPackageLock(lock).instances.toArray().map((i) => [`${i.name}@${i.version}`, i]));
    expect(byKey.get("mkdirp@0.5.1")?.dev).toBe(true);
    expect(byKey.get("minimist@0.0.8")?.dev).toBe(true);
    expect(byKey.get("minimist@1.2.0")?.dev).toBe(false);
    expect(byKey.get("optional-thing@2.0.0")?.dev).toBe(false);
    expect([...byKey.keys()].some((k) => k.startsWith("app@"))).toBe(false);
  });

  it("marks only the hoisted copy direct in the audit set; the nested copy is a known-dev transitive", () => {
    write("package.json", JSON.stringify({ dependencies: { minimist: "^1.2.0" } }));
    write("package-lock.json", JSON.stringify(lock));
    const audit = resolveAuditVersions(dir, ["minimist"], [], "javascript");
    const direct = audit.find((d) => d.name === "minimist" && d.version === "1.2.0")!;
    const nested = audit.find((d) => d.name === "minimist" && d.version === "0.0.8")!;
    expect(direct.isDirect).toBe(true);
    expect(nested.isDirect).toBe(false);
    expect(nested.devScopeKnown).toBe(true);
    expect(nested.isDev).toBe(true);
  });

  it("walks a v1 lockfile's nested dependencies", () => {
    const { versions, instances } = readPackageLock({
      dependencies: {
        a: { version: "1.0.0", dependencies: { b: { version: "2.0.0", dev: true } } },
        b: { version: "3.0.0" },
      },
    });
    expect(versions.get("b")).toBe("3.0.0");
    expect(instances.toArray().map((i) => `${i.name}@${i.version}`).sort()).toEqual(["a@1.0.0", "b@2.0.0", "b@3.0.0"]);
  });
});

describe("Cargo.lock: two versions of one crate, workspace members excluded", () => {
  const lock = `
version = 3

[[package]]
name = "fourda"
version = "1.0.2"
dependencies = [
 "rsa 0.10.0-rc.18",
 "serde",
]

[[package]]
name = "relay"
version = "0.1.0"
dependencies = [
 "rsa 0.9.10",
]

[[package]]
name = "rsa"
version = "0.9.10"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aa"

[[package]]
name = "rsa"
version = "0.10.0-rc.18"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
source = "registry+https://github.com/rust-lang/crates.io-index"
version = "1.0.228"
`;

  it("parses fields in any order", () => {
    const serde = parseCargoLockPackages(lock).find((p) => p.name === "serde")!;
    expect(serde.version).toBe("1.0.228");
    expect(serde.source).toContain("crates.io-index");
  });

  it("scans BOTH rsa versions and never the workspace's own crates", () => {
    write("Cargo.lock", lock);
    const read = resolveVersionSource(dir, "crates.io");
    const rsa = read.instances.filter((i) => i.name === "rsa").map((i) => i.version).sort();
    expect(rsa).toEqual(["0.10.0-rc.18", "0.9.10"]);
    expect(read.instances.some((i) => i.name === "fourda" || i.name === "relay")).toBe(false);
    // The direct version is the one the first member names.
    expect(read.versions.get("rsa")).toBe("0.10.0-rc.18");
    expect(read.versions.get("serde")).toBe("1.0.228");
  });
});

describe("pnpm and yarn: every copy", () => {
  it("pnpm v6: both versions of a package, with the dev flag", () => {
    const content = [
      "lockfileVersion: '6.0'",
      "dependencies:",
      "  minimist:",
      "    specifier: ^1.2.0",
      "    version: 1.2.0",
      "packages:",
      "",
      "  /minimist@0.0.8:",
      "    resolution: {integrity: x}",
      "    dev: true",
      "",
      "  /minimist@1.2.0:",
      "    resolution: {integrity: y}",
      "    dev: false",
      "",
    ].join("\n");
    const versions = new Map<string, string>();
    const instances = new InstanceSet();
    readPnpmLock(content, versions, instances);
    expect(versions.get("minimist")).toBe("1.2.0");
    const byVersion = new Map(instances.toArray().filter((i) => i.name === "minimist").map((i) => [i.version, i.dev]));
    expect(byVersion.get("0.0.8")).toBe(true);
    expect(byVersion.get("1.2.0")).toBe(false);
  });

  it("yarn: the direct copy is the one the package.json range resolves to, not the first block", () => {
    write("package.json", JSON.stringify({ dependencies: { minimist: "^1.2.0" } }));
    write(
      "yarn.lock",
      [
        "minimist@0.0.8:",
        '  version "0.0.8"',
        "",
        'minimist@^1.2.0, minimist@^1.2.5:',
        '  version "1.2.8"',
        "",
      ].join("\n"),
    );
    const read = resolveVersionSource(dir, "npm");
    expect(read.versions.get("minimist")).toBe("1.2.8");
    expect(read.instances.map((i) => i.version).sort()).toEqual(["0.0.8", "1.2.8"]);
    const specs = new Map<string, string>();
    readYarnLock(fs.readFileSync(path.join(dir, "yarn.lock"), "utf8"), new Map(), undefined, specs);
    expect(specs.get("minimist@^1.2.5")).toBe("1.2.8");
  });
});

describe("Python", () => {
  it("requirements.txt: extras, dotted names, ===, markers, comments, continuations; ranges skipped", () => {
    const parsed = parseRequirements(
      [
        "urllib3[secure]==1.25.0",
        "zope.interface==5.4.0  # needed by twisted",
        "Pillow==8.0.0 # imaging",
        "Django===3.2.0 ; python_version >= '3.6'",
        "requests==2.25.0 \\",
        "    --hash=sha256:abc",
        "flask>=1.0",
        "-r other.txt",
        "# a comment",
        "git+https://example.com/x.git#egg=x",
      ].join("\n"),
    );
    expect(parsed).toEqual([
      { name: "urllib3", version: "1.25.0" },
      { name: "zope-interface", version: "5.4.0" },
      { name: "pillow", version: "8.0.0" },
      { name: "django", version: "3.2.0" },
      { name: "requests", version: "2.25.0" },
    ]);
  });

  it("poetry.lock before 1.0 (alphabetical keys) and its dev category", () => {
    const { instances } = readTomlPackageLock(
      [
        "[[package]]",
        'category = "main"',
        'description = "x"',
        'name = "PyYAML"',
        "optional = false",
        'version = "5.3"',
        "",
        "[[package]]",
        'category = "dev"',
        'name = "pytest"',
        'version = "5.4.0"',
        "",
        "[package.dependencies]",
        'six = "*"',
        "",
        "[metadata]",
        'content-hash = "x"',
      ].join("\n"),
    );
    expect(instances.toArray()).toEqual([
      { name: "pyyaml", version: "5.3", dev: false },
      { name: "pytest", version: "5.4.0", dev: true },
    ]);
  });

  it("uv.lock: the project itself is not a release", () => {
    const { instances } = readTomlPackageLock(
      [
        "[[package]]",
        'name = "myapp"',
        'version = "0.1.0"',
        'source = { editable = "." }',
        "",
        "[[package]]",
        'name = "httpx"',
        'version = "0.27.0"',
        'source = { registry = "https://pypi.org/simple" }',
      ].join("\n"),
    );
    expect(instances.toArray().map((i) => i.name)).toEqual(["httpx"]);
  });

  it("pyproject: PEP 621, Poetry and dependency-groups, PEP 503 names", () => {
    const parsed = parsePyprojectDependencies(
      [
        "[project]",
        'name = "x"',
        "dependencies = [",
        '  "requests[socks]>=2.0; python_version > \'3\'",',
        '  "Typing_Extensions",',
        "]",
        "",
        "[tool.poetry.dependencies]",
        'python = "^3.10"',
        'Django = "^4.2"',
        "",
        "[tool.poetry.group.dev.dependencies]",
        'pytest = "^8"',
        "",
        "[dependency-groups]",
        'lint = ["ruff>=0.4"]',
      ].join("\n"),
    );
    expect(parsed.deps.sort()).toEqual(["django", "requests", "typing-extensions"]);
    expect(parsed.devDeps.sort()).toEqual(["pytest", "ruff"]);
  });
});

describe("Go", () => {
  const goMod = [
    "module example.com/app",
    "",
    "go 1.21",
    "",
    "require (",
    "\tgolang.org/x/net v0.0.0-20200622213623-75b288015ac9",
    "\tgithub.com/docker/docker v20.10.0+incompatible // indirect",
    "\tgithub.com/old/thing v1.0.0",
    ")",
    "",
    "require github.com/single/line v1.2.3",
    "",
    "replace github.com/old/thing => github.com/new/thing v1.1.0",
    "replace github.com/local/x => ./x",
  ].join("\n");

  it("keeps full pseudo-versions and +incompatible, applies replace, marks indirect", () => {
    const { requires, goVersionAtLeast117 } = parseGoMod(goMod);
    expect(goVersionAtLeast117).toBe(true);
    const byModule = new Map(requires.map((r) => [r.module, r]));
    expect(byModule.get("golang.org/x/net")?.version).toBe("v0.0.0-20200622213623-75b288015ac9");
    expect(byModule.get("github.com/docker/docker")?.version).toBe("v20.10.0+incompatible");
    expect(byModule.get("github.com/docker/docker")?.indirect).toBe(true);
    expect(byModule.get("github.com/new/thing")?.version).toBe("v1.1.0");
    expect(byModule.has("github.com/old/thing")).toBe(false);
    expect(byModule.get("github.com/single/line")?.version).toBe("v1.2.3");
  });

  it("go >= 1.17 reads go.mod alone; older go.mod is completed from go.sum's selected versions", () => {
    write("go.mod", goMod);
    write("go.sum", "example.com/extra v1.0.0 h1:x=\nexample.com/extra v1.1.0 h1:y=\n");
    expect(resolveVersionSource(dir, "Go").instances.some((i) => i.name === "example.com/extra")).toBe(false);

    write("go.mod", goMod.replace("go 1.21", "go 1.13"));
    const read = resolveVersionSource(dir, "Go");
    expect(read.instances.find((i) => i.name === "example.com/extra")?.version).toBe("v1.1.0");
  });

  it("go.sum: the highest version recorded, /go.mod-only lines included (Go's build list)", () => {
    const selected = selectedFromGoSum(
      [
        "github.com/a/b v1.0.0 h1:x=",
        "github.com/a/b v1.0.0/go.mod h1:x=",
        "github.com/a/b v1.2.0/go.mod h1:y=",
        "google.golang.org/grpc v1.21.0/go.mod h1:q=",
        "golang.org/x/sys v0.0.0-20200101000000-aaaaaaaaaaaa h1:z=",
        "golang.org/x/sys v0.0.0-20210101000000-bbbbbbbbbbbb h1:w=",
      ].join("\n"),
    );
    expect(selected.get("github.com/a/b")).toBe("v1.2.0");
    expect(selected.get("google.golang.org/grpc")).toBe("v1.21.0");
    expect(selected.get("golang.org/x/sys")).toBe("v0.0.0-20210101000000-bbbbbbbbbbbb");
    expect(compareGoVersions("v0.0.0-20210101000000-bbbbbbbbbbbb", "v0.1.0")).toBe(-1);
  });
});

describe("version ordering per ecosystem", () => {
  it("PEP 440: two-part releases, pre/post/dev, epochs", () => {
    const order = ["1.0.dev1", "1.0a1", "1.0a1.post1", "1.0b2", "1.0rc1", "1.0", "1.0.post1", "1.1", "1!0.5"];
    for (let i = 1; i < order.length; i++) {
      expect(compareVersions(order[i - 1], order[i], "PyPI"), `${order[i - 1]} < ${order[i]}`).toBe(-1);
    }
    expect(compareVersions("5.1", "5.1.0", "PyPI")).toBe(0);
    expect(parsePep440("not-a-version")).toBeNull();
  });

  it("Go: v-prefixed installed versions against v-less OSV bounds, pseudo-versions as prereleases", () => {
    expect(compareVersions("v1.4.1", "1.4.2", "Go")).toBe(-1);
    expect(compareVersions("v0.0.0-20200622213623-75b288015ac9", "0.0.0-20201021035429-f5854403a974", "Go")).toBe(-1);
    expect(compareVersions("0", "v0.0.1", "Go")).toBe(-1);
    expect(isComparable("v20.10.0+incompatible", "Go")).toBe(true);
  });
});

describe("fix selection, the PyPI way", () => {
  const django: OsvVulnerability["affected"] = [
    {
      package: { name: "Django", ecosystem: "PyPI" },
      ranges: [
        {
          type: "ECOSYSTEM",
          events: [
            { introduced: "3.2" },
            { fixed: "3.2.25" },
            { introduced: "4.2" },
            { fixed: "4.2.16" },
            { introduced: "5.1" },
            { fixed: "5.1.1" },
          ],
        },
      ],
    },
  ];

  it("django 3.2.0 gets its own line's fix (3.2.25), not the 5.1 line's", () => {
    // `compareSemver` read "3.2" and "5.1" as unparseable and therefore equal.
    expect(extractFixedVersion(django, "django", "PyPI", "3.2.0")).toBe("3.2.25");
  });

  it("matches advisory package names case- and punctuation-insensitively for PyPI", () => {
    const jinja: OsvVulnerability["affected"] = [
      { package: { name: "Jinja2", ecosystem: "PyPI" }, ranges: [{ type: "ECOSYSTEM", events: [{ introduced: "0" }, { fixed: "3.1.6" }] }] },
    ];
    expect(extractFixedVersion(jinja, "jinja2", "PyPI", "2.11.0")).toBe("3.1.6");
  });
});

describe("project discovery", () => {
  it("a Cargo virtual workspace contributes its members' dependencies, not the members", () => {
    write("crates/a/Cargo.toml", '[package]\nname = "a"\n\n[dependencies]\nserde = "1"\nb = { path = "../b" }\n');
    write("crates/b/Cargo.toml", '[package]\nname = "b"\n\n[dev-dependencies]\ntempfile = "3"\n');
    const deps = cargoWorkspaceDeps(dir, '[workspace]\nmembers = ["crates/*"]\n\n[workspace.dependencies]\ntokio = "1"\n');
    expect(deps.deps.sort()).toEqual(["serde", "tokio"]);
    expect(deps.devDeps).toEqual(["tempfile"]);
  });

  it("skips directories and lockfiles the repository ignores, nested .gitignore and info/exclude included", () => {
    write(".gitignore", "# scratch\ncli/\n/victauri-gauntlet\n*.tmp\n!keep.tmp\n");
    write(".git/info/exclude", "scratch-clone/\n");
    write("cli/pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    write("victauri-gauntlet/Cargo.lock", "");
    write("scratch-clone/package-lock.json", "{}");
    write("proto/.gitignore", "Cargo.lock\n");
    write("proto/Cargo.lock", "");
    write("proto/Cargo.toml", '[package]\nname = "proto"\n');
    write("relay/Cargo.lock", '[[package]]\nname = "rsa"\nversion = "0.9.10"\nsource = "registry+x"\n');
    const rel = scanProjectTree(dir).map((t) => path.relative(dir, t.dir).replace(/\\/g, "/") || ".").sort();
    expect(rel).toEqual([".", "relay"]);
  });

  it("finds independently-locked projects below the root and skips installed code and fixtures", () => {
    write("package.json", JSON.stringify({ dependencies: { react: "^19" } }));
    write("package-lock.json", JSON.stringify({ packages: { "node_modules/react": { version: "19.0.0" } } }));
    write("src-tauri/Cargo.toml", '[package]\nname = "app"\n\n[dependencies]\nserde = "1"\n');
    write("src-tauri/Cargo.lock", '[[package]]\nname = "serde"\nversion = "1.0.0"\nsource = "registry+x"\n');
    write("node_modules/x/package-lock.json", "{}");
    write("tests/fixtures/old/package-lock.json", "{}");
    write("tools/requirements.txt", "requests==2.25.0\n");

    const tree = scanProjectTree(dir);
    const rel = tree.map((t) => path.relative(dir, t.dir).replace(/\\/g, "/") || ".").sort();
    expect(rel).toEqual([".", "src-tauri", "tools"]);
    const groups = treeResolutionGroups(tree);
    expect(groups.map((g) => g.language).sort()).toEqual(["npm", "python", "rust"]);
  });
});
