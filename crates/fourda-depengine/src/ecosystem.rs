// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The subset of the app's ecosystem normalisation (`src-tauri/src/ecosystem.rs`)
//! the pure rules need. A parity test in the app
//! (`osv::depengine_parity_tests`) holds the two alias tables together.

/// The OSV ecosystem identifier for any alias the app stores (registry name,
/// ACE language name, common shorthand), case-insensitively. `None` for an
/// ecosystem OSV does not index or an unknown string. The app's
/// `Ecosystem::parse(alias).map(Ecosystem::osv_name)`.
pub fn osv_name(alias: &str) -> Option<&'static str> {
    match alias.trim().to_lowercase().as_str() {
        "npm" | "javascript" | "typescript" | "node" | "js" | "ts" => Some("npm"),
        "rust" | "cargo" | "crates.io" | "crates" => Some("crates.io"),
        "python" | "pypi" | "pip" | "py" => Some("PyPI"),
        "go" | "golang" => Some("Go"),
        "java" | "maven" | "kotlin" | "gradle" => Some("Maven"),
        "csharp" | "c#" | "dotnet" | "nuget" => Some("NuGet"),
        "php" | "composer" | "packagist" => Some("Packagist"),
        "ruby" | "rubygems" | "gem" => Some("RubyGems"),
        "dart" | "flutter" | "pub" => Some("Pub"),
        _ => None,
    }
}

/// [`osv_name`], with an unknown ecosystem passed through unchanged: the key
/// the matcher joins advisories and installed copies on.
pub fn normalize(ecosystem: &str) -> &str {
    osv_name(ecosystem).unwrap_or(ecosystem)
}

/// Whether `ecosystem` names PyPI, under any alias the app uses.
pub fn is_pypi(ecosystem: &str) -> bool {
    osv_name(ecosystem) == Some("PyPI")
}
