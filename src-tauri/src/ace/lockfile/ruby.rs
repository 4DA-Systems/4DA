// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `Gemfile.lock`: the gems Bundler installs from a rubygems remote.
//!
//! The reader this replaces (2026-10-10) read the `specs:` list of EVERY
//! source section and kept the platform in the version. Measured on the
//! conformance corpus (rails 6.0): the 11 gems of rails' own `PATH` source
//! and 2 `GIT` checkouts were read as registry installs at their local
//! version, and 29 platform builds (`nokogiri (1.9.1-x64-mingw32)`) became
//! versions no advisory range parses — RubyGems findings precision 0.43.
//!
//! - Only a `GEM` section names registry releases. `GIT` (a checkout at a
//!   revision), `PATH` (local code) and `PLUGIN SOURCE` specs are counted as
//!   non-registry entries, never sent to OSV as RubyGems.
//! - A spec line is `    name (version)` or `    name (version-platform)`.
//!   Bundler's own lockfile parser (`Bundler::LockfileParser::NAME_VERSION`)
//!   takes the version as everything before the first `-` and the platform
//!   as the rest; this reader splits the same way. The platform builds of one
//!   release are one installed version, so `(name, version)` is kept once.

use std::collections::HashSet;
use std::path::PathBuf;

use super::{LockFormat, LockedPackage, LockfileRead};

/// The source section a spec line sits under.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    /// A rubygems remote (`GEM`).
    Registry,
    /// `GIT`, `PATH`, `PLUGIN SOURCE`: specs that are not registry releases.
    Local,
    /// `PLATFORMS`, `DEPENDENCIES`, `CHECKSUMS`, … — no specs.
    Other,
}

fn section_of(header: &str) -> Section {
    match header.trim_end() {
        "GEM" => Section::Registry,
        "GIT" | "PATH" | "PLUGIN SOURCE" => Section::Local,
        _ => Section::Other,
    }
}

/// `name (version)` / `name (version-platform)` -> `(name, version)`.
fn spec_line(trimmed: &str) -> Option<(&str, &str)> {
    let (name, rest) = trimmed.split_once(" (")?;
    let inside = rest.strip_suffix(')')?;
    let version = inside.split('-').next().unwrap_or(inside).trim();
    let name = name.trim();
    if name.is_empty() || name.contains(' ') || version.is_empty() {
        return None;
    }
    Some((name, version))
}

pub(super) fn read_gemfile_lock(content: &str) -> LockfileRead {
    let mut read = LockfileRead {
        path: PathBuf::new(),
        format: LockFormat::Gemfile,
        packages: Vec::new(),
        edges: Vec::new(),
        non_registry_entries: 0,
    };
    let mut section = Section::Other;
    let mut in_specs = false;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            section = section_of(line);
            in_specs = false;
            continue;
        }
        if line.trim_end() == "  specs:" {
            in_specs = true;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 2 {
            in_specs = false; // `  remote:`, `  revision:`, …
            continue;
        }
        // Specs sit at 4 spaces; their own requirements at 6.
        if !in_specs || indent != 4 {
            continue;
        }
        let Some((name, version)) = spec_line(line.trim()) else {
            continue;
        };
        match section {
            Section::Registry => {
                if seen.insert((name.to_string(), version.to_string())) {
                    let first = !read.packages.iter().any(|p| p.name == name);
                    read.packages
                        .push(LockedPackage::new(name, version).primary(first));
                }
            }
            Section::Local => read.non_registry_entries += 1,
            Section::Other => {}
        }
    }
    read
}
