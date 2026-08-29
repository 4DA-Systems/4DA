// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Canonical registry of language builtin / standard-library module names.
//!
//! THE single home for "is this import a builtin, not a package?" — consumed
//! by the import-scrape path in [`crate::ace::scanner`] (so builtins are never
//! persisted as user dependencies) and by the startup self-heal purge in
//! `db::dependencies` (so rows import-scraped before this fix existed are
//! removed from existing installs).
//!
//! Why this matters: the import scraper merges any imported module name into a
//! project's dependency list with `version = NULL, is_direct = 1`. Node
//! builtins (`fs`, `path`, `http`, ...) and Python stdlib modules (`os`,
//! `json`, ...) are not packages — they can never be version-resolved, never
//! match a registry, and their generic names collide with real advisories
//! ("http" minted a phantom "Security: http" decision window on live data).
//!
//! CRITICAL distinction the purge relies on: real registry packages that
//! SHARE a builtin name must survive —
//! - npm polyfills (`buffer@5.7.1`, `events@3.3.0`, `string_decoder@1.3.0`)
//!   arrive from lockfiles WITH versions (and transitives with `is_direct=0`);
//! - the Rust `http` / `url` CRATES live in `ecosystem='rust'`.
//! The pollution signature is exactly: builtin name + `version IS NULL` +
//! `is_direct = 1` in a javascript/python ecosystem row.

use std::collections::HashSet;
use std::sync::LazyLock;

/// Node.js builtin modules (top-level specifier form, without the `node:`
/// prefix). Covers every builtin through Node 22; prefix-only builtins
/// (`node:test`, `node:sqlite`, `node:sea`) are handled by the `node:` prefix
/// rule in [`is_node_builtin`], not this list.
const NODE_BUILTIN_MODULES: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// Python standard-library modules (top-level import names). Not exhaustive of
/// every dot-release, but covers the stdlib surface a real codebase imports;
/// an unlisted stdlib straggler costs one junk row, while a listed name can
/// only suppress an import-scrape guess (manifest/lockfile parsing is
/// unaffected, so declared backports like PyPI `dataclasses` still persist).
const PYTHON_STDLIB_MODULES: &[&str] = &[
    "abc",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "atexit",
    "base64",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "calendar",
    "cmath",
    "codecs",
    "collections",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "cprofile",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "doctest",
    "email",
    "enum",
    "errno",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "functools",
    "gc",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "multiprocessing",
    "netrc",
    "numbers",
    "operator",
    "os",
    "pathlib",
    "pdb",
    "pickle",
    "pkgutil",
    "platform",
    "plistlib",
    "pprint",
    "profile",
    "pty",
    "pwd",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtplib",
    "socket",
    "socketserver",
    "sqlite3",
    "ssl",
    "stat",
    "statistics",
    "string",
    "struct",
    "subprocess",
    "sys",
    "sysconfig",
    "syslog",
    "tarfile",
    "tempfile",
    "termios",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "xml",
    "xmlrpc",
    "zipfile",
    "zlib",
    "zoneinfo",
];

/// Host-ambient JS modules: import specifiers resolved by the RUNTIME hosting
/// the code, not by npm. `import * as vscode from "vscode"` in an extension
/// binds the editor's own API — there is nothing to install, no version to
/// resolve, and the same-named npm package is a deprecated shim. Observed live
/// 2026-08-30: the import scraper recorded `vscode` as a direct, version-null
/// npm dependency of `editors/vscode/4da`, and every dependency surface
/// downstream (get_context, dependency_health, blind spots) inherited a
/// phantom deprecated dep. `electron` behaves identically inside a renderer.
const HOST_AMBIENT_MODULES: &[&str] = &["vscode", "electron"];

static NODE_BUILTIN_SET: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    NODE_BUILTIN_MODULES
        .iter()
        .chain(HOST_AMBIENT_MODULES.iter())
        .copied()
        .collect()
});

static PYTHON_STDLIB_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| PYTHON_STDLIB_MODULES.iter().copied().collect());

/// True when a JS/TS import specifier names a Node builtin or a host-ambient
/// module ([`HOST_AMBIENT_MODULES`]). ANY `node:`-prefixed
/// specifier is a builtin by definition (npm package names cannot contain `:`);
/// bare specifiers are checked against the builtin list. Case-sensitive on
/// purpose for bare names (npm names are lowercase; builtins are too), but the
/// purge path lowercases before calling, which is a no-op for these names.
pub(crate) fn is_node_builtin(specifier: &str) -> bool {
    if specifier.strip_prefix("node:").is_some() {
        // The node: namespace is builtin-only by definition (npm package
        // names cannot contain ':'), so ANY node:-prefixed specifier is a
        // builtin — including prefix-only ones like node:test / node:sqlite
        // that have no bare form and are deliberately absent from the list.
        return true;
    }
    NODE_BUILTIN_SET.contains(specifier)
}

/// True when a Python top-level import name is a standard-library module.
pub(crate) fn is_python_stdlib(module: &str) -> bool {
    PYTHON_STDLIB_SET.contains(module)
}

/// True when a Go import path is standard library. Canonical Go rule (used by
/// `go` tooling itself): stdlib import paths have no dot in their first path
/// segment (`fmt`, `net/http`, `encoding/json`), while module paths start with
/// a domain (`github.com/...`, `golang.org/...`).
pub(crate) fn is_go_stdlib_import(import_path: &str) -> bool {
    !import_path.split('/').next().unwrap_or("").contains('.')
}

/// Go stdlib LAST-SEGMENT names, for the LEGACY (provenance-unknown) purge
/// arm only. Pre-fix go import-scraped rows were stored by last path segment
/// (`net/http` -> "http", `encoding/json` -> "json"), so the full-path
/// [`is_go_stdlib_import`] rule cannot classify them — and applying its
/// no-dot heuristic to bare names would purge every legitimate go module row
/// ("gin", "cobra"). This curated list carries the same documented one-shot
/// churn tradeoff as the rest of the legacy arm: a real module whose last
/// segment collides (github.com/pkg/errors -> "errors") is purged once, then
/// re-scraped with provenance='import_scrape' and immune thereafter.
const GO_STDLIB_LAST_SEGMENTS: &[&str] = &[
    "bufio", "bytes", "context", "crypto", "embed", "encoding", "errors", "flag", "fmt", "http",
    "io", "json", "log", "math", "net", "os", "path", "filepath", "reflect", "regexp", "runtime",
    "sort", "strconv", "strings", "sync", "syscall", "testing", "time", "unicode", "unsafe", "url",
    "xml",
];

static GO_STDLIB_LAST_SEGMENT_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| GO_STDLIB_LAST_SEGMENTS.iter().copied().collect());

/// True when a bare (last-segment) go dependency NAME is a stdlib package.
/// Full module paths (containing '/') never match — `github.com/x/errors`
/// stored as a full path is a real module row.
pub(crate) fn is_go_stdlib_name(name: &str) -> bool {
    !name.contains('/') && GO_STDLIB_LAST_SEGMENT_SET.contains(name)
}

/// The canonical purge predicate: is `package_name` (already lowercased by the
/// caller or naturally lowercase) a builtin for the given dependency-table
/// ecosystem label? ONLY javascript and python ecosystems participate — a
/// `http` row in `ecosystem='rust'` is the real http CRATE and must never
/// match. Ecosystem aliases mirror the dedup fold in
/// `db::dependencies::queries` ('npm' etc. appear in older rows).
pub(crate) fn is_builtin_for_ecosystem(ecosystem: &str, package_name: &str) -> bool {
    let name = package_name.to_lowercase();
    match ecosystem.to_lowercase().as_str() {
        "javascript" | "js" | "npm" | "node" => is_node_builtin(&name),
        "python" | "py" | "pypi" | "pip" => is_python_stdlib(&name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_builtins_match_bare_and_prefixed() {
        for name in ["fs", "path", "http", "child_process", "worker_threads"] {
            assert!(is_node_builtin(name), "bare builtin must match: {name}");
            assert!(
                is_node_builtin(&format!("node:{name}")),
                "prefixed builtin must match: node:{name}"
            );
        }
        // Prefix-only builtins are covered by the node: rule alone.
        assert!(is_node_builtin("node:test"));
        assert!(is_node_builtin("node:sqlite"));
    }

    #[test]
    fn node_real_packages_do_not_match() {
        for name in ["react", "express", "axios", "left-pad", "@scope/http"] {
            assert!(
                !is_node_builtin(name),
                "real package must not match: {name}"
            );
        }
    }

    #[test]
    fn python_stdlib_matches_and_packages_do_not() {
        for name in ["os", "sys", "json", "sqlite3", "zoneinfo", "asyncio"] {
            assert!(is_python_stdlib(name), "stdlib must match: {name}");
        }
        for name in ["numpy", "flask", "requests", "django"] {
            assert!(!is_python_stdlib(name), "package must not match: {name}");
        }
    }

    #[test]
    fn go_stdlib_first_segment_dot_rule() {
        for p in ["fmt", "net/http", "encoding/json", "os", "strings"] {
            assert!(is_go_stdlib_import(p), "stdlib import: {p}");
        }
        for p in [
            "github.com/gin-gonic/gin",
            "golang.org/x/tools",
            "k8s.io/api",
        ] {
            assert!(!is_go_stdlib_import(p), "module import: {p}");
        }
    }

    #[test]
    fn go_stdlib_last_segment_names_for_legacy_purge() {
        for n in ["fmt", "http", "json", "os", "errors", "strings"] {
            assert!(is_go_stdlib_name(n), "stdlib last segment: {n}");
        }
        // Real modules and full paths never match.
        for n in ["gin", "cobra", "github.com/pkg/errors", "golang.org/x/text"] {
            assert!(!is_go_stdlib_name(n), "must not match: {n}");
        }
    }

    #[test]
    fn ecosystem_predicate_scopes_to_js_and_python_only() {
        assert!(is_builtin_for_ecosystem("javascript", "http"));
        assert!(is_builtin_for_ecosystem("javascript", "node:fs"));
        // Host-ambient modules: provided by the hosting runtime, never
        // installable from npm at the imported name.
        assert!(is_builtin_for_ecosystem("javascript", "vscode"));
        assert!(is_builtin_for_ecosystem("javascript", "electron"));
        assert!(!is_builtin_for_ecosystem("rust", "vscode"));
        assert!(is_builtin_for_ecosystem("python", "os"));
        // The rust http CRATE and any other ecosystem never match.
        assert!(!is_builtin_for_ecosystem("rust", "http"));
        assert!(!is_builtin_for_ecosystem("go", "http"));
        assert!(!is_builtin_for_ecosystem("rust", "url"));
        // Real packages in the scoped ecosystems don't match either.
        assert!(!is_builtin_for_ecosystem("javascript", "react"));
        assert!(!is_builtin_for_ecosystem("python", "numpy"));
    }
}
