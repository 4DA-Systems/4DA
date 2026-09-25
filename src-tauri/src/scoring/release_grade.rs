// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Release grading (v37): a registry release is graded against the version
//! EACH project pins, and the projects it concerns are named.
//!
//! Before v37 every release of a user dependency was one undifferentiated
//! "New release in your stack", ranked by the words in its description. Live
//! 2026-09-25, after the harvest Phase 0 fix let releases reach the corpus:
//!
//! - `crates.io: fastembed v7.1.0` (4DA pins 5.13.4, two majors behind, the
//!   embedding backbone) scored 0.155 and left the feed, while seven
//!   `tauri 3.0.0-alpha` rows held 0.90.
//! - Over 45 days, prereleases were kept 9 of 12 times and breaking stable
//!   upgrades 13 of 24.
//! - `crates.io: sha2 v0.11.0` read "installed v0.11.0" (the 4DA copy) while
//!   two sibling projects, on 0.10.9, were the ones it actually concerned.
//!
//! The grade answers the two questions an operator asks of a release: WHICH
//! projects does it concern, and WHAT should they do. Semver defines the
//! answer: a new major (or, below 1.0, a new minor) is a breaking upgrade; a
//! new minor is worth knowing; a patch is not news on its own (security fixes
//! reach the user through the advisory lanes, not the release row), and
//! neither is a registry prerelease row (the announcement reaches the user
//! through editorial coverage). A pinned version that has been YANKED is
//! the strongest case of all: the project is running something the publisher
//! withdrew.
//!
//! Only projects that declare the package DIRECTLY can act on it. A project
//! that reaches the package only transitively is upgraded by its parent, so it
//! never makes a release actionable.

use semver::Version;

use crate::db::Database;

use super::dependencies;
use super::release_version::lenient_semver;

/// How far one project's pinned copy is behind the announced release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReleaseGap {
    /// Same major.minor, newer patch.
    Patch,
    /// Newer minor on the same major (1.x and above).
    Minor,
    /// Newer major, or a newer minor below 1.0 (Cargo's compatibility rule).
    Breaking,
}

/// The grade that decides how a release row is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReleaseClass {
    /// A project runs a version the publisher yanked.
    Yanked,
    /// A breaking upgrade is available to a project that declares it directly.
    Breaking,
    /// A new minor is available to a project that declares it directly.
    Minor,
    /// Only a patch is available, or only transitive copies are behind.
    Patch,
    /// The announced version is a prerelease (alpha, beta, rc, ...).
    Prerelease,
}

/// One project's copy of the released package.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProjectPin {
    pub project_path: String,
    pub installed: Version,
    pub is_direct: bool,
    pub is_dev: bool,
    /// `None` when the project already runs the announced version or newer.
    pub gap: Option<ReleaseGap>,
    /// The project's pinned version appears in the registry's yanked list.
    pub yanked: bool,
}

/// A registry release graded against every project that carries its package.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReleaseGrade {
    pub package: String,
    pub announced: Version,
    pub pins: Vec<ProjectPin>,
}

/// The gap between an installed version and an announced one, or `None` when
/// the installed copy is already at or past it.
pub(crate) fn gap(installed: &Version, announced: &Version) -> Option<ReleaseGap> {
    if announced <= installed {
        return None;
    }
    if announced.major != installed.major
        || (installed.major == 0 && announced.minor != installed.minor)
    {
        return Some(ReleaseGap::Breaking);
    }
    if announced.minor != installed.minor {
        return Some(ReleaseGap::Minor);
    }
    Some(ReleaseGap::Patch)
}

/// The versions a crates.io row lists after `Yanked versions:` (the adapter
/// writes that line into the item body; it is the only place the list
/// survives the DB boundary).
pub(crate) fn yanked_versions(content: &str) -> Vec<String> {
    content
        .lines()
        .find_map(|l| l.trim().strip_prefix("Yanked versions:"))
        .map(|rest| {
            rest.split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

impl ReleaseGrade {
    /// Build a grade from raw pins. `pins` carries (project, installed version
    /// string, is_direct, is_dev); unparseable versions are dropped, and a
    /// project listed twice keeps its DIRECT copy (then its lowest version),
    /// because that is the copy the project can act on.
    pub(crate) fn from_pins(
        package: &str,
        announced: Version,
        raw_pins: Vec<(String, String, bool, bool)>,
        yanked: &[String],
    ) -> Self {
        let mut pins: Vec<ProjectPin> = Vec::new();
        for (path, raw_version, is_direct, is_dev) in raw_pins {
            let Some(installed) = lenient_semver(&raw_version, None) else {
                continue;
            };
            let key = path.replace('\\', "/").to_lowercase();
            let pin = ProjectPin {
                gap: gap(&installed, &announced),
                yanked: yanked.iter().any(|y| y.trim() == raw_version.trim()),
                project_path: path,
                installed,
                is_direct,
                is_dev,
            };
            match pins
                .iter_mut()
                .find(|p| p.project_path.replace('\\', "/").to_lowercase() == key)
            {
                Some(existing) => {
                    let better = (pin.is_direct && !existing.is_direct)
                        || (pin.is_direct == existing.is_direct
                            && pin.installed < existing.installed);
                    if better {
                        *existing = pin;
                    }
                }
                None => pins.push(pin),
            }
        }
        pins.sort_by(|a, b| a.project_path.cmp(&b.project_path));
        Self {
            package: package.to_string(),
            announced,
            pins,
        }
    }

    /// Every project already runs the announced version or newer (the v33
    /// already-installed rule). An empty pin set is "cannot tell" → false.
    pub(crate) fn all_installed(&self) -> bool {
        !self.pins.is_empty() && self.pins.iter().all(|p| p.gap.is_none())
    }

    /// Projects that declare the package directly and are behind (or yanked).
    fn actionable_pins(&self) -> impl Iterator<Item = &ProjectPin> {
        self.pins
            .iter()
            .filter(|p| p.is_direct && (p.gap.is_some() || p.yanked))
    }

    /// The class that decides the row's treatment, or `None` when the grade
    /// has nothing to say (no pins, or every project already current).
    pub(crate) fn class(&self) -> Option<ReleaseClass> {
        let any_yanked = self.pins.iter().any(|p| p.is_direct && p.yanked);
        if self.pins.is_empty() || (self.all_installed() && !any_yanked) {
            return None;
        }
        if any_yanked {
            return Some(ReleaseClass::Yanked);
        }
        if !self.announced.pre.is_empty() {
            return Some(ReleaseClass::Prerelease);
        }
        match self.actionable_pins().filter_map(|p| p.gap).max() {
            Some(ReleaseGap::Breaking) => Some(ReleaseClass::Breaking),
            Some(ReleaseGap::Minor) => Some(ReleaseClass::Minor),
            Some(ReleaseGap::Patch) | None => Some(ReleaseClass::Patch),
        }
    }

    /// A breaking upgrade (or a yanked pin) for a project that declares the
    /// package as a RUNTIME dependency: the case that should change what a
    /// project does, so it always reaches the feed. A dev-only pin (a test
    /// runner, a linter) is tooling news, left to its score.
    pub(crate) fn actionable_runtime(&self) -> bool {
        matches!(
            self.class(),
            Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
        ) && self.concerned_pins().iter().any(|p| !p.is_dev)
    }

    /// The projects the class is about: the yanked ones for `Yanked`, else
    /// the direct projects carrying the largest gap. Sorted, de-duplicated.
    pub(crate) fn concerned_pins(&self) -> Vec<&ProjectPin> {
        let class = self.class();
        let mut out: Vec<&ProjectPin> = match class {
            Some(ReleaseClass::Yanked) => self
                .pins
                .iter()
                .filter(|p| p.is_direct && p.yanked)
                .collect(),
            Some(ReleaseClass::Prerelease) => self
                .pins
                .iter()
                .filter(|p| p.is_direct && p.gap.is_some())
                .collect(),
            Some(_) => {
                let top = self.actionable_pins().filter_map(|p| p.gap).max();
                match top {
                    Some(g) => self
                        .actionable_pins()
                        .filter(|p| p.gap == Some(g))
                        .collect(),
                    None => self.pins.iter().filter(|p| p.gap.is_some()).collect(),
                }
            }
            None => Vec::new(),
        };
        out.sort_by(|a, b| a.project_path.cmp(&b.project_path));
        out
    }

    /// The installed version the row should display: the lowest concerned
    /// copy (the one furthest behind).
    pub(crate) fn headline_installed(&self) -> Option<String> {
        self.concerned_pins()
            .iter()
            .map(|p| &p.installed)
            .min()
            .map(ToString::to_string)
    }

    /// "4da/src-tauri on 5.13.4", "work/tools (+4 more) on 0.10.9",
    /// or with mixed versions "… on 0.10.9–0.10.12".
    pub(crate) fn concerned_label(&self) -> Option<String> {
        let pins = self.concerned_pins();
        let paths: Vec<String> = pins.iter().map(|p| p.project_path.clone()).collect();
        let location = dependencies::project_label(&paths)?;
        let lo = pins.iter().map(|p| &p.installed).min()?;
        let hi = pins.iter().map(|p| &p.installed).max()?;
        Some(if lo == hi {
            format!("{location} on {lo}")
        } else {
            format!("{location} on {lo}\u{2013}{hi}")
        })
    }

    /// Projects that already run the announced version or newer, as a label
    /// ("4da/src-tauri (+1 more)"), for the explanation's contrast clause.
    pub(crate) fn current_label(&self) -> Option<String> {
        let paths: Vec<String> = self
            .pins
            .iter()
            .filter(|p| p.gap.is_none() && !p.yanked)
            .map(|p| p.project_path.clone())
            .collect();
        dependencies::project_label(&paths)
    }

    /// Necessity reason, one line, naming the package, the release and the
    /// projects it concerns.
    pub(crate) fn necessity_reason(&self) -> Option<String> {
        let class = self.class()?;
        let who = self.concerned_label();
        let pkg = &self.package;
        let new = &self.announced;
        Some(match (class, who) {
            (ReleaseClass::Yanked, Some(w)) => {
                format!("{pkg}: the pinned version was yanked ({w}); {new} is current")
            }
            (ReleaseClass::Yanked, None) => format!("{pkg}: a pinned version was yanked"),
            (ReleaseClass::Breaking, Some(w)) => format!("Breaking upgrade: {pkg} {new} for {w}"),
            (ReleaseClass::Breaking, None) => format!("Breaking upgrade: {pkg} {new}"),
            (ReleaseClass::Minor, Some(w)) => format!("New {pkg} {new} for {w}"),
            (ReleaseClass::Minor, None) => format!("New {pkg} {new}"),
            (ReleaseClass::Patch, Some(w)) => format!("Patch release {pkg} {new} ({w})"),
            (ReleaseClass::Patch, None) => format!("Patch release {pkg} {new}"),
            (ReleaseClass::Prerelease, Some(w)) => {
                format!("Pre-release {pkg} {new} ({w} stays on stable)")
            }
            (ReleaseClass::Prerelease, None) => format!("Pre-release {pkg} {new}"),
        })
    }

    /// The dependency factor's headline and evidence for the explanation
    /// chain: (display, evidence).
    pub(crate) fn chain_text(&self) -> Option<(String, String)> {
        let class = self.class()?;
        let pkg = &self.package;
        let new = &self.announced;
        let display = match class {
            ReleaseClass::Yanked => format!("Your pinned {pkg} was yanked"),
            ReleaseClass::Breaking => format!("Breaking upgrade of your dependency {pkg}"),
            ReleaseClass::Minor => format!("New release of your dependency {pkg}"),
            ReleaseClass::Patch => format!("Patch release of your dependency {pkg}"),
            ReleaseClass::Prerelease => format!("Pre-release of your dependency {pkg}"),
        };
        let mut evidence = format!("{pkg} {new}");
        if let Some(who) = self.concerned_label() {
            evidence.push_str(&format!(" \u{b7} {who}"));
        }
        if let Some(current) = self.current_label() {
            evidence.push_str(&format!(" \u{b7} already current: {current}"));
        }
        evidence.push_str(" \u{2014} the subject of this release");
        Some((display, evidence))
    }
}

/// Grade a registry release row from its title ("crates.io: fastembed
/// v7.1.0") against `user_dependencies`. Reads the same rows, with the same
/// filters, as the v33 already-installed rule (`pipeline_v2`): every included
/// project, the registry's own manifest language, `-`/`_` equivalent names.
/// `None` for non-registry sources, unparseable titles, or no known pins.
pub(crate) fn grade_registry_release(
    db: &Database,
    source_type: &str,
    title: &str,
    content: &str,
) -> Option<ReleaseGrade> {
    if !crate::dep_linker::is_registry_source(source_type) {
        return None;
    }
    let (subject, version) = crate::dep_linker::registry_title_subject(title)?;
    let announced = lenient_semver(&version?, None)?;
    let lang = dependencies::registry_manifest_language(source_type)?;
    let raw_pins = load_pins(db, &subject, lang);
    if raw_pins.is_empty() {
        return None;
    }
    let yanked = if matches!(source_type, "crates_io" | "crates") {
        yanked_versions(content)
    } else {
        Vec::new()
    };
    Some(ReleaseGrade::from_pins(
        &subject, announced, raw_pins, &yanked,
    ))
}

/// (project_path, version, is_direct, is_dev) for every included project that
/// carries `subject` in `lang`.
pub(crate) fn load_pins(
    db: &Database,
    subject: &str,
    lang: &str,
) -> Vec<(String, String, bool, bool)> {
    let conn = db.conn.lock();
    let Ok(mut stmt) = conn.prepare_cached(
        "SELECT project_path, version, ecosystem, is_direct, is_dev FROM user_dependencies
         WHERE LOWER(REPLACE(package_name, '_', '-')) = LOWER(REPLACE(?1, '_', '-'))
           AND version IS NOT NULL AND version <> ''",
    ) else {
        return Vec::new();
    };
    let rows: Vec<(String, String, String, bool, bool)> = stmt
        .query_map(rusqlite::params![subject], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get::<_, i64>(4)? != 0,
            ))
        })
        .map(|rs| rs.flatten().collect())
        .unwrap_or_default();
    drop(stmt);
    drop(conn);
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    rows.into_iter()
        .filter(|(path, _, eco, _, _)| {
            dependencies::ecosystem_congruent(lang, eco)
                && !crate::project_inclusion::is_excluded_from_intelligence(path, &user_excluded)
        })
        .map(|(path, v, _, direct, dev)| (path, v, direct, dev))
        .collect()
}

#[cfg(test)]
#[path = "release_grade_tests.rs"]
mod tests;
