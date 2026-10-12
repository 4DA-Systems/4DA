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
