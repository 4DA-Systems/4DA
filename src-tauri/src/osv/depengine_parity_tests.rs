// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The pure engine crate carries its own copy of the one ecosystem rule it
//! needs (is this alias PyPI?). This holds the copy to the app's
//! `crate::ecosystem::Ecosystem`, so an alias added on one side alone fails.

use crate::ecosystem::Ecosystem;

#[test]
fn depengine_pypi_aliases_match_the_app() {
    let candidates = [
        "python",
        "pypi",
        "PyPI",
        " pip ",
        "py",
        "PY",
        "npm",
        "javascript",
        "rust",
        "cargo",
        "crates.io",
        "go",
        "golang",
        "maven",
        "nuget",
        "composer",
        "packagist",
        "rubygems",
        "gem",
        "dart",
        "pub",
        "",
        "conda",
        "pipenv",
        "poetry",
    ];
    for alias in candidates {
        let app = Ecosystem::parse(alias).map(Ecosystem::osv_name) == Some("PyPI");
        assert_eq!(
            fourda_depengine::ecosystem::is_pypi(alias),
            app,
            "is_pypi({alias:?}) disagrees with crate::ecosystem"
        );
    }
}

/// The matcher (C0.2) joins advisories and copies on the crate's alias
/// table; it must be the app's, alias for alias.
#[test]
fn depengine_osv_names_match_the_app() {
    let candidates = [
        "npm",
        "javascript",
        "TypeScript",
        "node",
        "js",
        "ts",
        "rust",
        "cargo",
        "crates.io",
        "crates",
        "python",
        "PyPI",
        " pip ",
        "py",
        "go",
        "golang",
        "java",
        "maven",
        "kotlin",
        "gradle",
        "csharp",
        "C#",
        "dotnet",
        "nuget",
        "php",
        "composer",
        "packagist",
        "ruby",
        "rubygems",
        "gem",
        "dart",
        "flutter",
        "pub",
        "",
        "swift",
        "advisory",
        "conda",
    ];
    for alias in candidates {
        assert_eq!(
            fourda_depengine::ecosystem::osv_name(alias),
            Ecosystem::parse(alias).map(Ecosystem::osv_name),
            "osv_name({alias:?}) disagrees with crate::ecosystem"
        );
    }
}
