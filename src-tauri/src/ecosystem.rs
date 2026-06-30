// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Canonical package-ecosystem identity — the single source of truth for
//! ecosystem/language alias recognition.
//!
//! Before this module, every site that reasoned about an ecosystem (OSV vuln
//! matching, CVE/NVD matching, source-adapter routing, the source-fetching
//! registry comparison, blind-spot display labels) hand-maintained its OWN
//! `match` over alias strings — `"npm" | "javascript" | "typescript" => ...`.
//! Those lists drifted: one knew `"composer"` as a PHP alias, another didn't; the
//! CVE matcher was missing C#/PHP/Dart entirely; "Pub" was absent from places it
//! belonged. That drift caused real, security-relevant matching gaps.
//!
//! The fix is one [`Ecosystem::parse`] that recognizes every alias once, plus
//! per-consumer accessors. The accessors deliberately return DIFFERENT vocab —
//! OSV uses `"PyPI"`, the CVE matcher uses `"pip"`, source-fetching uses
//! `"pypi"` — because each talks to a different external database and that
//! contract must not silently change. What is unified is *recognition* (the
//! input side), which is where the drift actually lived. Adding an alias or
//! ecosystem is now a one-line change here that every consumer picks up.

/// A package ecosystem that OSV indexes and our matchers understand.
///
/// Swift/CocoaPods and C/C++ are intentionally NOT variants — OSV does not index
/// them, so [`Ecosystem::parse`] returns `None` for them and every consumer keeps
/// its existing fallback (passthrough / `None` / osv-only / a display-only label).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecosystem {
    Npm,
    Cargo,
    PyPI,
    Go,
    Maven,
    NuGet,
    Packagist,
    RubyGems,
    Pub,
}

impl Ecosystem {
    /// Recognize an ecosystem from any alias — registry name, ACE language name,
    /// or common shorthand — case-insensitively. THE single source of truth for
    /// "which strings mean this ecosystem". Returns `None` for ecosystems OSV does
    /// not index (Swift, C/C++) or genuinely unknown input, so callers fall back
    /// to their own default behavior.
    pub fn parse(alias: &str) -> Option<Self> {
        match alias.trim().to_lowercase().as_str() {
            "npm" | "javascript" | "typescript" | "node" | "js" | "ts" => Some(Self::Npm),
            "rust" | "cargo" | "crates.io" | "crates" => Some(Self::Cargo),
            "python" | "pypi" | "pip" | "py" => Some(Self::PyPI),
            "go" | "golang" => Some(Self::Go),
            "java" | "maven" | "kotlin" | "gradle" => Some(Self::Maven),
            "csharp" | "c#" | "dotnet" | "nuget" => Some(Self::NuGet),
            "php" | "composer" | "packagist" => Some(Self::Packagist),
            "ruby" | "rubygems" | "gem" => Some(Self::RubyGems),
            "dart" | "flutter" | "pub" => Some(Self::Pub),
            _ => None,
        }
    }

    /// The OSV ecosystem identifier (see the OSV schema's `affected[].package.ecosystem`).
    /// Used by the OSV advisory matchers. OSV uses mixed case ("PyPI", "Go", "Maven").
    pub fn osv_name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Cargo => "crates.io",
            Self::PyPI => "PyPI",
            Self::Go => "Go",
            Self::Maven => "Maven",
            Self::NuGet => "NuGet",
            Self::Packagist => "Packagist",
            Self::RubyGems => "RubyGems",
            Self::Pub => "Pub",
        }
    }

    /// Human-facing registry label for blind-spot / dependency display
    /// (e.g. "react (npm)", "laravel/framework (Packagist)").
    pub fn display_label(self) -> &'static str {
        // Happens to equal `osv_name` today, but kept distinct: display copy and
        // the OSV wire identifier are different contracts and may diverge.
        match self {
            Self::Npm => "npm",
            Self::Cargo => "crates.io",
            Self::PyPI => "PyPI",
            Self::Go => "Go",
            Self::Maven => "Maven",
            Self::NuGet => "NuGet",
            Self::Packagist => "Packagist",
            Self::RubyGems => "RubyGems",
            Self::Pub => "Pub",
        }
    }

    /// Source adapters that cover this ecosystem, in priority order. Every
    /// ecosystem is covered by OSV + GitHub advisories; some also have a
    /// dedicated registry adapter that leads.
    pub fn source_types(self) -> Vec<String> {
        let mut v: Vec<String> = match self {
            Self::Npm => vec!["npm_registry".into()],
            Self::Cargo => vec!["crates_io".into()],
            Self::PyPI => vec!["pypi".into()],
            Self::Go => vec!["go_modules".into()],
            Self::Maven | Self::NuGet | Self::Packagist | Self::RubyGems | Self::Pub => vec![],
        };
        v.push("osv".into());
        v.push("github".into());
        v
    }

    /// Lowercase token used by source-fetching to compare a dependency's
    /// ecosystem against a registry adapter's key. Distinct from `osv_name`
    /// (e.g. "crates" not "crates.io", "pypi" not "PyPI") — all lowercase.
    pub fn fetch_token(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Cargo => "crates",
            Self::PyPI => "pypi",
            Self::Go => "go",
            Self::Maven => "maven",
            Self::NuGet => "nuget",
            Self::Packagist => "packagist",
            Self::RubyGems => "rubygems",
            Self::Pub => "pub",
        }
    }

    /// Normalization token used by the CVE/NVD matcher. Distinct from `osv_name`
    /// because the GitHub Advisory / NVD comparison vocab differs ("pip" not
    /// "PyPI", and lowercase). Both sides of every CVE comparison run through
    /// this, so the value only needs to be internally consistent — which is
    /// exactly why centralizing the *aliases* here (and not the output) is safe.
    pub fn cve_token(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Cargo => "crates.io",
            Self::PyPI => "pip",
            Self::Go => "go",
            Self::Maven => "maven",
            Self::NuGet => "nuget",
            Self::Packagist => "packagist",
            Self::RubyGems => "rubygems",
            Self::Pub => "pub",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recognizes_every_alias() {
        for a in [
            "npm",
            "javascript",
            "typescript",
            "node",
            "js",
            "ts",
            "NPM",
            "  Npm ",
        ] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Npm), "alias {a}");
        }
        for a in ["rust", "cargo", "crates.io", "crates"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Cargo), "alias {a}");
        }
        for a in ["python", "pypi", "pip", "py"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::PyPI), "alias {a}");
        }
        for a in ["go", "golang"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Go), "alias {a}");
        }
        for a in ["java", "maven", "kotlin", "gradle"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Maven), "alias {a}");
        }
        for a in ["csharp", "c#", "dotnet", "nuget"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::NuGet), "alias {a}");
        }
        for a in ["php", "composer", "packagist"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Packagist), "alias {a}");
        }
        for a in ["ruby", "rubygems", "gem"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::RubyGems), "alias {a}");
        }
        for a in ["dart", "flutter", "pub"] {
            assert_eq!(Ecosystem::parse(a), Some(Ecosystem::Pub), "alias {a}");
        }
    }

    #[test]
    fn parse_returns_none_for_unindexed_or_unknown() {
        // OSV does not index these — callers must keep their fallback behavior.
        for a in [
            "swift",
            "cocoapods",
            "cpp",
            "c",
            "elixir",
            "",
            "   ",
            "made-up",
        ] {
            assert_eq!(Ecosystem::parse(a), None, "input {a:?}");
        }
    }

    #[test]
    fn osv_names_match_the_osv_schema() {
        assert_eq!(Ecosystem::Npm.osv_name(), "npm");
        assert_eq!(Ecosystem::Cargo.osv_name(), "crates.io");
        assert_eq!(Ecosystem::PyPI.osv_name(), "PyPI");
        assert_eq!(Ecosystem::Go.osv_name(), "Go");
        assert_eq!(Ecosystem::Maven.osv_name(), "Maven");
        assert_eq!(Ecosystem::NuGet.osv_name(), "NuGet");
        assert_eq!(Ecosystem::Packagist.osv_name(), "Packagist");
        assert_eq!(Ecosystem::RubyGems.osv_name(), "RubyGems");
        assert_eq!(Ecosystem::Pub.osv_name(), "Pub");
    }

    #[test]
    fn cve_token_preserves_the_nvd_vocab() {
        // These six MUST match the pre-consolidation cve_matching outputs.
        assert_eq!(Ecosystem::Npm.cve_token(), "npm");
        assert_eq!(Ecosystem::Cargo.cve_token(), "crates.io");
        assert_eq!(Ecosystem::PyPI.cve_token(), "pip");
        assert_eq!(Ecosystem::Go.cve_token(), "go");
        assert_eq!(Ecosystem::RubyGems.cve_token(), "rubygems");
        assert_eq!(Ecosystem::Maven.cve_token(), "maven");
        // These three were MISSING before (the bug): C#/PHP/Dart now collapse
        // consistently so a dep alias and an advisory alias normalize equal.
        assert_eq!(Ecosystem::NuGet.cve_token(), "nuget");
        assert_eq!(Ecosystem::Packagist.cve_token(), "packagist");
        assert_eq!(Ecosystem::Pub.cve_token(), "pub");
    }

    #[test]
    fn fetch_tokens_are_lowercase_registry_keys() {
        assert_eq!(Ecosystem::Cargo.fetch_token(), "crates");
        assert_eq!(Ecosystem::PyPI.fetch_token(), "pypi");
        assert_eq!(Ecosystem::Npm.fetch_token(), "npm");
    }

    #[test]
    fn source_types_lead_with_the_dedicated_adapter_then_osv_github() {
        assert_eq!(
            Ecosystem::Npm.source_types(),
            ["npm_registry", "osv", "github"]
        );
        assert_eq!(
            Ecosystem::Cargo.source_types(),
            ["crates_io", "osv", "github"]
        );
        assert_eq!(Ecosystem::PyPI.source_types(), ["pypi", "osv", "github"]);
        assert_eq!(
            Ecosystem::Go.source_types(),
            ["go_modules", "osv", "github"]
        );
        // Ecosystems without a dedicated registry adapter: OSV + GitHub only.
        assert_eq!(Ecosystem::Maven.source_types(), ["osv", "github"]);
        assert_eq!(Ecosystem::NuGet.source_types(), ["osv", "github"]);
        assert_eq!(Ecosystem::Packagist.source_types(), ["osv", "github"]);
    }
}
