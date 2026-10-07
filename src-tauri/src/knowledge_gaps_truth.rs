// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which projects a knowledge gap is TRUE for.
//!
//! A gap is either (a) a live, directly-declaring project behind a release
//! from the last 30 days — graded per project by `scoring::release_grade`
//! exactly as the brief's upgrade facts are (ecosystem-aware; dormant,
//! scratch and excluded projects skipped) — or (b) an advisory whose affected
//! range still covers a named project's installed version. Editorial
//! coverage alone stays a low-confidence citation.
//!
//! Live 2026-10-07 the `chrono 0.4.45` gap named "d:/4da/relay (+8 more)"
//! although relay, src-tauri and a third workspace already lock 0.4.45 —
//! only the victauri copies on 0.4.44 lag — and `notify 8.2.0` named
//! src-tauri (already 8.2.0) instead of the real laggards, a bridge app on
//! 6.1.1 and victauri-cli on 7.0.0.
//! The old rule grouped projects by package name, showed the first
//! project's version, and dropped a release only when EVERY project was
//! current, with the ecosystem lost on the way.

// UTF-8 safety gate, as in the parent module.
#![deny(clippy::string_slice)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use semver::Version;

use crate::evidence::ProjectLiveness;
use crate::scoring::release_grade::{self, PinRow, ReleaseClass, ReleaseGrade};
use crate::scoring::release_version::{announced_release_version, lenient_semver};

use super::MissedItem;

/// `-`/`_`-insensitive, lowercase — the key `release_grade::load_pins` matches on.
fn canon(name: &str) -> String {
    name.replace('_', "-").to_lowercase()
}

/// A dependency's manifest language as a registry language
/// (`ecosystem_congruent` accepts TypeScript under JavaScript).
pub(super) fn registry_lang(language: &str) -> String {
    match language.to_lowercase().as_str() {
        "typescript" => "javascript".to_string(),
        other => other.to_string(),
    }
}

/// Every project's pinned copy of every package, loaded once per pass, plus
/// the liveness facts the brief's upgrade facts filter on.
pub(super) struct PinBook {
    by_name: HashMap<String, Vec<PinRow>>,
    dormant: HashSet<String>,
    excluded: Vec<String>,
    liveness: ProjectLiveness,
    own: RefCell<HashMap<(String, String, String), bool>>,
}

impl PinBook {
    pub(super) fn load(conn: &rusqlite::Connection) -> Self {
        let mut by_name: HashMap<String, Vec<PinRow>> = HashMap::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT package_name, project_path, version, ecosystem, is_direct, is_dev
             FROM user_dependencies WHERE version IS NOT NULL AND version <> ''",
        ) {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    (
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)? != 0,
                        r.get::<_, i64>(5)? != 0,
                    ),
                ))
            }) {
                for (name, row) in rows.flatten() {
                    by_name.entry(canon(&name)).or_default().push(row);
                }
            }
        }
        Self {
            by_name,
            dormant: crate::ace::dormancy::dormant_project_paths(conn),
            excluded: crate::project_inclusion::user_excluded_paths(),
            liveness: ProjectLiveness::load(conn),
            own: RefCell::new(HashMap::new()),
        }
    }

    /// Test constructor: rows are (package, path, version, ecosystem, direct, dev).
    #[cfg(test)]
    pub(super) fn from_rows(
        rows: &[(&str, &str, &str, &str, bool, bool)],
        liveness: ProjectLiveness,
    ) -> Self {
        let mut by_name: HashMap<String, Vec<PinRow>> = HashMap::new();
        for (name, path, version, eco, direct, dev) in rows {
            by_name.entry(canon(name)).or_default().push((
                (*path).to_string(),
                (*version).to_string(),
                (*eco).to_string(),
                *direct,
                *dev,
            ));
        }
        Self {
            by_name,
            dormant: HashSet::new(),
            excluded: Vec::new(),
            liveness,
            own: RefCell::new(HashMap::new()),
        }
    }

    fn is_own(&self, path: &str, package: &str, lang: &str) -> bool {
        let key = (path.to_string(), canon(package), lang.to_string());
        if let Some(hit) = self.own.borrow().get(&key) {
            return *hit;
        }
        let own = crate::scoring::release_ownership::is_own_package(path, package, lang);
        self.own.borrow_mut().insert(key, own);
        own
    }

    fn active(&self, path: &str) -> bool {
        !self.liveness.is_scratch(path) && !self.liveness.is_dormant(path)
    }

    fn grade(
        &self,
        subject: &str,
        announced: Version,
        lang: &str,
        yanked: &[String],
    ) -> Option<ReleaseGrade> {
        let rows = self
            .by_name
            .get(&canon(subject))
            .cloned()
            .unwrap_or_default();
        let pins = release_grade::filter_pin_rows(rows, lang, &self.dormant, &self.excluded);
        release_grade::grade_pins(subject, announced, pins, yanked, |path| {
            self.is_own(path, subject, lang)
        })
    }
}

/// One project's copy a release leaves behind.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BehindPin {
    pub path: String,
    pub installed: Version,
}

/// What a candidate row says about this dependency.
enum RowVerdict {
    /// A release that live, directly-declaring projects are behind.
    Behind {
        class: ReleaseClass,
        announced: Version,
        pins: Vec<BehindPin>,
    },
    /// A release no live project is behind, a prerelease, or a namesake in
    /// another ecosystem: not a gap and not a citation.
    NotNews,
    /// No version evidence either way: kept as a citation.
    Citation,
}

/// The rows that survive release grading and who they leave behind.
#[derive(Debug, Default)]
pub(super) struct ReleaseTriage {
    pub kept: Vec<MissedItem>,
    /// Distinct projects, each at its lowest behind version, sorted by path.
    pub behind: Vec<BehindPin>,
    /// The newest release that leaves a project behind.
    pub latest: Option<Version>,
    /// The most consequential class among those releases.
    pub worst: Option<ReleaseClass>,
}

fn class_rank(class: ReleaseClass) -> u8 {
    match class {
        ReleaseClass::Yanked => 0,
        ReleaseClass::Breaking => 1,
        ReleaseClass::Minor => 2,
        ReleaseClass::Patch => 3,
        ReleaseClass::Prerelease => 4,
    }
}

impl ReleaseTriage {
    fn absorb_behind(&mut self, class: ReleaseClass, announced: Version, pins: Vec<BehindPin>) {
        if self.worst.is_none_or(|w| class_rank(class) < class_rank(w)) {
            self.worst = Some(class);
        }
        if self.latest.as_ref().is_none_or(|l| announced > *l) {
            self.latest = Some(announced);
        }
        for pin in pins {
            let key = pin.path.replace('\\', "/").to_lowercase();
            match self
                .behind
                .iter_mut()
                .find(|b| b.path.replace('\\', "/").to_lowercase() == key)
            {
                Some(existing) if pin.installed < existing.installed => *existing = pin,
                Some(_) => {}
                None => self.behind.push(pin),
            }
        }
        self.behind.sort_by(|a, b| a.path.cmp(&b.path));
    }
}

/// Grade every matched row against each project's own pin. `langs` are the
/// registry languages the dependency name is declared in.
pub(super) fn triage_releases(
    conn: &rusqlite::Connection,
    book: &PinBook,
    dep_name: &str,
    langs: &[String],
    missed: Vec<MissedItem>,
) -> ReleaseTriage {
    let mut triage = ReleaseTriage::default();
    for m in missed {
        match row_verdict(conn, book, dep_name, langs, &m) {
            RowVerdict::Behind {
                class,
                announced,
                pins,
            } => {
                triage.absorb_behind(class, announced, pins);
                triage.kept.push(m);
            }
            RowVerdict::Citation => triage.kept.push(m),
            RowVerdict::NotNews => {}
        }
    }
    triage
}

fn row_verdict(
    conn: &rusqlite::Connection,
    book: &PinBook,
    dep_name: &str,
    langs: &[String],
    m: &MissedItem,
) -> RowVerdict {
    if crate::dep_linker::is_registry_source(&m.source_type) {
        let Some((subject, version)) = crate::dep_linker::registry_title_subject(&m.title) else {
            return RowVerdict::Citation;
        };
        // Another package's release that mentions this name.
        if !crate::dep_linker::registry_names_equal(&subject, dep_name) {
            return RowVerdict::Citation;
        }
        let announced = version.as_deref().and_then(|v| lenient_semver(v, None));
        let lang = release_grade::registry_manifest_language(&m.source_type);
        let (Some(announced), Some(lang)) = (announced, lang) else {
            return RowVerdict::Citation;
        };
        let yanked = if matches!(m.source_type.as_str(), "crates_io" | "crates") {
            release_grade::yanked_versions(&item_content(conn, m.item_id))
        } else {
            Vec::new()
        };
        // No project carries the package in THIS registry's ecosystem: the
        // row is a cross-ecosystem namesake, never a gap.
        return book
            .grade(&subject, announced, lang, &yanked)
            .map_or(RowVerdict::NotNews, |g| verdict_from_grade(book, &g));
    }
    let Some(announced) = announced_release_version(&m.title, &m.source_type, dep_name) else {
        return RowVerdict::Citation;
    };
    // An editorial version is only comparable when the name lives in ONE
    // ecosystem; across two it is ambiguous, so it stays a citation.
    let [lang] = langs else {
        return RowVerdict::Citation;
    };
    // Editorial text may only DROP a row (every live project already runs
    // what it announces, AD-041) — never prove a project behind. Its version
    // parse is a guess: live 2026-10-07 "UUID v4 vs. UUID v7 vs. ULID" read
    // as uuid 4.0.0 and put eleven Rust projects "behind uuid 4.0.0". Only a
    // registry row's structured subject + version makes a Release gap.
    match book
        .grade(dep_name, announced, lang, &[])
        .map(|g| verdict_from_grade(book, &g))
    {
        Some(RowVerdict::NotNews) => RowVerdict::NotNews,
        _ => RowVerdict::Citation,
    }
}

fn verdict_from_grade(book: &PinBook, grade: &ReleaseGrade) -> RowVerdict {
    let Some(class) = grade.class() else {
        return RowVerdict::NotNews;
    };
    if class == ReleaseClass::Prerelease {
        return RowVerdict::NotNews;
    }
    let pins: Vec<BehindPin> = grade
        .concerned_pins()
        .into_iter()
        .filter(|p| p.is_direct && book.active(&p.project_path))
        .map(|p| BehindPin {
            path: p.project_path.clone(),
            installed: p.installed.clone(),
        })
        .collect();
    if pins.is_empty() {
        return RowVerdict::NotNews;
    }
    RowVerdict::Behind {
        class,
        announced: grade.announced.clone(),
        pins,
    }
}

/// A registry row's body (the yanked list lives there). Best-effort.
fn item_content(conn: &rusqlite::Connection, item_id: i64) -> String {
    conn.query_row(
        "SELECT COALESCE(content, '') FROM source_items WHERE id = ?1",
        [item_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_default()
}

/// "0.4.44", or "0.4.44–0.4.45" when the attributed copies differ. `None`
/// when nothing parses.
pub(super) fn version_label<'v>(versions: impl IntoIterator<Item = &'v str>) -> Option<String> {
    let mut parsed: Vec<(Version, &str)> = versions
        .into_iter()
        .filter_map(|v| lenient_semver(v, None).map(|p| (p, v)))
        .collect();
    parsed.sort_by(|a, b| a.0.cmp(&b.0));
    let (lo, hi) = (parsed.first()?, parsed.last()?);
    Some(if lo.0 == hi.0 {
        lo.1.to_string()
    } else {
        format!("{}\u{2013}{}", lo.1, hi.1)
    })
}

/// The live subset of `paths` (dormant and scratch projects dropped); every
/// path when none is known-live, so attribution never comes back empty.
pub(super) fn live_paths(book: &PinBook, paths: &[String]) -> Vec<String> {
    let live: Vec<String> = paths.iter().filter(|p| book.active(p)).cloned().collect();
    if live.is_empty() {
        paths.to_vec()
    } else {
        live
    }
}

#[cfg(test)]
#[path = "knowledge_gaps_truth_tests.rs"]
mod tests;
