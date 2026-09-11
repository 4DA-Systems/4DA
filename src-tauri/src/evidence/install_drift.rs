// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Pinned is not installed (AD-046).
//!
//! Every dependency surface reads versions from the LOCKFILE —
//! `dependency_instances` is written by the lockfile walk. The code that runs
//! is whatever sits in `node_modules`. Live 2026-09-09/10:
//! `mcp-4da-server/pnpm-lock.yaml` pinned hono 4.13.5 from 21:43Z on the 9th,
//! while `mcp-4da-server/node_modules/hono` stayed at 4.13.1 — exposed to
//! CVE-2026-84363/-84364/-84365 — for 25 days, because activation ran
//! `pnpm install` at the repository root only. `dependency_instances` said
//! 4.13.5, so Preemption, Blind Spots, the brief and the MCP all reported the
//! fix as done while the code that loads was still vulnerable. The product
//! had no notion of "installed" distinct from "pinned".
//!
//! This is a LIVE read, not a stored column (AD-046 records why): for every
//! active project with direct npm installs, resolve each direct dependency
//! the way Node does and compare its `version` with the pin. ONE row per
//! project, because the action — one install command — is per project. Never
//! installed is not drift; an unreadable manifest is skipped, never an error.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::dormant_notice::{project_leaf, truncate_title};
use super::liveness::ProjectLiveness;
use super::types::{
    Action, Confidence, EvidenceCitation, EvidenceItem, EvidenceKind, LensHints, Urgency,
};
use super::upgrade_plan::truncate;

/// Id prefix of every install-drift row. The row is identified by it, not by
/// a `LensHints` field: every new hint must be listed in the exhaustive
/// `LensHints` literal in `knowledge_decay.rs`, a file a peer PR holds
/// (AD-046, Open). `preemption` and the list transport key on it.
pub const ID_PREFIX: &str = "install-drift:";

/// True for an install-drift row — a statement about what a PROJECT's
/// `node_modules` holds, which no per-package surface may subsume.
pub fn is_install_drift_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

/// Parent directories searched above the project for a hoisted
/// `node_modules`. The repository root (the first directory holding `.git`)
/// ends the search sooner.
const MAX_PARENT_LEVELS: usize = 6;

/// A `package.json` larger than this is not a manifest worth parsing.
const MAX_MANIFEST_BYTES: u64 = 1 << 20;

/// Citation caps: the title and explanation carry the full counts.
const MAX_ADVISORY_CITATIONS: usize = 3;
const MAX_PACKAGE_CITATIONS: usize = 4;

/// `validate_item` caps a citation note at 200 BYTES.
const MAX_NOTE_BYTES: usize = 200;

/// `validate_item` caps a title at 120 BYTES.
const MAX_TITLE_BYTES: usize = 120;

/// The lockfile whose pins `dependency_instances` holds for a project.
///
/// The walk runs the package-lock, then the pnpm, then the yarn processor, and
/// each REPLACES the project's npm instance set — so the last one present
/// wrote the pins, and the install command names that lockfile's tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lockfile {
    Npm,
    Pnpm,
    Yarn,
}

impl Lockfile {
    fn detect(dir: &Path) -> Option<Self> {
        [Self::Yarn, Self::Pnpm, Self::Npm]
            .into_iter()
            .find(|kind| dir.join(kind.file_name()).is_file())
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Npm => "package-lock.json",
            Self::Pnpm => "pnpm-lock.yaml",
            Self::Yarn => "yarn.lock",
        }
    }

    /// The command that makes `node_modules` match this lockfile again.
    pub fn install_command(self) -> &'static str {
        match self {
            Self::Npm => "npm ci",
            Self::Pnpm => "pnpm install",
            Self::Yarn => "yarn install",
        }
    }
}

/// An npm advisory the running copy is exposed to.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvisoryRef {
    pub id: String,
    pub summary: String,
    pub url: Option<String>,
}

/// The copy of a package Node would load.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledCopy {
    pub version: String,
    pub manifest: PathBuf,
}

/// One direct dependency whose installed copy is not what the lockfile pins.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageDrift {
    pub package: String,
    /// Every version the lockfile pins for it (almost always one).
    pub pinned: BTreeSet<String>,
    /// What Node would load; `None` when no `node_modules` searched holds it.
    pub installed: Option<InstalledCopy>,
    /// Advisories affecting the installed copy and NO pinned version — the
    /// install clears them: the fix is merged but not running.
    pub cleared_by_install: Vec<AdvisoryRef>,
    /// Advisories affecting the installed copy AND a pinned version.
    pub still_exposed: Vec<AdvisoryRef>,
    /// Some stored npm advisory affects a pinned version (whether or not it
    /// affects the installed copy) — the pin is not a clean target to name.
    pub pin_exposed: bool,
    /// Every pinned instance of it is dev-only (`ace::dep_scope`).
    pub dev_only: bool,
}

impl PackageDrift {
    /// 0 = the fix is merged but not running, 1 = exposed either way,
    /// 2 = drift without exposure, 3 = not installed at all.
    fn rank(&self) -> u8 {
        if !self.cleared_by_install.is_empty() {
            0
        } else if !self.still_exposed.is_empty() {
            1
        } else if self.installed.is_some() {
            2
        } else {
            3
        }
    }

    fn urgency(&self) -> Urgency {
        match self.rank() {
            0 => Urgency::High,
            1 => Urgency::Medium,
            _ => Urgency::Watch,
        }
    }

    fn pinned_display(&self) -> String {
        self.pinned.iter().cloned().collect::<Vec<_>>().join("/")
    }

    /// `hono: 4.13.1 installed, 4.13.5 pinned` — the title's form.
    fn detail(&self) -> String {
        match &self.installed {
            Some(copy) => format!(
                "{}: {} installed, {} pinned",
                self.package,
                copy.version,
                self.pinned_display()
            ),
            None => format!(
                "{}: not installed, {} pinned",
                self.package,
                self.pinned_display()
            ),
        }
    }

    /// What the install would and would not change, in one sentence.
    fn exposure_sentence(&self, project_label: &str) -> Option<String> {
        let Some(copy) = &self.installed else {
            return Some(format!(
                "{pkg} is not installed anywhere between {project_label} and its repository \
                 root, so an import of it resolves outside the project or fails.",
                pkg = self.package
            ));
        };
        let pinned = self.pinned_display();
        if !self.cleared_by_install.is_empty() {
            let mut sentence = format!(
                "{pkg} {installed} is affected by {ids} and the pinned {pinned} is not \
                 — the fix is merged but not running.",
                pkg = self.package,
                installed = copy.version,
                ids = advisory_ids(&self.cleared_by_install),
            );
            if !self.still_exposed.is_empty() {
                sentence.push_str(&format!(
                    " {} affects the pinned {pinned} as well.",
                    advisory_ids(&self.still_exposed)
                ));
            }
            return Some(sentence);
        }
        if !self.still_exposed.is_empty() {
            return Some(format!(
                "{pkg} {installed} is affected by {ids}, and so is the pinned {pinned}: \
                 reinstalling syncs the code but does not clear it — the pin needs an upgrade too.",
                pkg = self.package,
                installed = copy.version,
                ids = advisory_ids(&self.still_exposed),
            ));
        }
        None
    }
}

/// One project whose `node_modules` disagrees with its lockfile.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectDrift {
    pub project: String,
    pub lockfile: Lockfile,
    /// Never empty. Most urgent first, then by name.
    pub packages: Vec<PackageDrift>,
}

impl ProjectDrift {
    /// High when the install would clear an advisory the running copy has;
    /// Medium when the running copy is exposed and the pin is too; Watch
    /// otherwise (drift without exposure, or a dependency not installed).
    pub fn urgency(&self) -> Urgency {
        self.packages
            .iter()
            .map(PackageDrift::urgency)
            .min()
            .unwrap_or(Urgency::Watch)
    }

    /// For a row whose install clears an advisory: the package, the version
    /// that runs, and the pinned version the install brings — but only when
    /// no stored advisory affects that pin, so it is a true fix to state.
    /// `None` otherwise: there is no fix that is honest to print.
    pub fn lead_fix(&self) -> Option<(&str, &str, &str)> {
        let lead = self
            .packages
            .iter()
            .find(|p| !p.cleared_by_install.is_empty() && !p.pin_exposed)?;
        let installed = lead.installed.as_ref()?;
        let pinned = lead.pinned.iter().max_by(|a, b| compare_versions(a, b))?;
        Some((
            lead.package.as_str(),
            installed.version.as_str(),
            pinned.as_str(),
        ))
    }

    /// Every drifted package is dev-only.
    pub fn all_dev_only(&self) -> bool {
        self.packages.iter().all(|p| p.dev_only)
    }

    /// The canonical Preemption row.
    pub fn to_evidence_item(&self) -> EvidenceItem {
        let label = project_leaf(&self.project);
        let urgency = self.urgency();
        let evidence = self.citations();
        let opens_advisory = evidence.first().is_some_and(|c| c.url.is_some());
        EvidenceItem {
            id: format!("{ID_PREFIX}{}", self.project),
            kind: EvidenceKind::Alert,
            title: self.title(label),
            explanation: self.explanation(label),
            // A High/Medium row rests on an OSV range match against the
            // installed version (read straight off disk); a Watch row on the
            // filesystem read alone.
            confidence: if urgency <= Urgency::Medium {
                Confidence::osv_verified(0.95)
            } else {
                Confidence::heuristic(0.95)
            },
            urgency,
            reversibility: None,
            evidence,
            evidence_total: None,
            affected_projects: vec![self.project.clone()],
            affected_deps: self.packages.iter().map(|p| p.package.clone()).collect(),
            suggested_actions: self.actions(label, opens_advisory),
            precedents: Vec::new(),
            refutation_condition: None,
            lens_hints: LensHints::preemption_only(),
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
        }
    }

    /// The specified form names the lead package; a row too long for the
    /// title budget names the count instead — never a word cut in half (live
    /// snapshot 2026-09-10: "... 20.11.0/26.4.0 pinn"). Truncation is the last
    /// resort, for a project name that alone blows the budget.
    fn title(&self, label: &str) -> String {
        let lockfile = self.lockfile.file_name();
        let lead = self
            .packages
            .first()
            .map(PackageDrift::detail)
            .unwrap_or_default();
        let others = self.packages.len().saturating_sub(1);
        let more = if others > 0 {
            format!(", +{others} more")
        } else {
            String::new()
        };
        let named = format!("{label} — node_modules is out of sync with {lockfile} ({lead}{more})");
        if named.len() <= MAX_TITLE_BYTES {
            return named;
        }
        let count = self.packages.len();
        let noun = if count == 1 { "package" } else { "packages" };
        truncate_title(format!(
            "{label} — node_modules is out of sync with {lockfile} ({count} {noun})"
        ))
    }

    fn explanation(&self, label: &str) -> String {
        let lockfile = self.lockfile.file_name();
        let count = self.packages.len();
        let details: Vec<String> = self
            .packages
            .iter()
            .map(|p| {
                let dev = if p.dev_only { " (dev-only)" } else { "" };
                match &p.installed {
                    Some(copy) => format!(
                        "{} {} installed, {} pinned{dev}",
                        p.package,
                        copy.version,
                        p.pinned_display()
                    ),
                    None => format!(
                        "{} not installed, {} pinned{dev}",
                        p.package,
                        p.pinned_display()
                    ),
                }
            })
            .collect();
        let mut text =
            format!(
            "{label}'s {lockfile} pins {count} direct {noun} at versions its node_modules does \
             not hold, so the code that actually loads is not the code the lockfile describes: \
             {details}.",
            noun = if count == 1 { "dependency" } else { "dependencies" },
            details = details.join("; "),
        );
        for sentence in self
            .packages
            .iter()
            .filter_map(|p| p.exposure_sentence(label))
        {
            text.push(' ');
            text.push_str(&sentence);
        }
        if self.urgency() == Urgency::Watch && self.packages.iter().any(|p| p.installed.is_some()) {
            text.push_str(
                " No installed copy here is affected by a known npm advisory: this is drift, \
                 not exposure.",
            );
        }
        text.push_str(&format!(
            " Run `{}` in {label} so node_modules matches {lockfile} again.",
            self.lockfile.install_command()
        ));
        text
    }

    /// Advisories first — a card's "Open advisory" opens the FIRST citation —
    /// then what `node_modules` holds, then what the lockfile pins.
    fn citations(&self) -> Vec<EvidenceCitation> {
        let mut out: Vec<EvidenceCitation> = self
            .packages
            .iter()
            .flat_map(|p| {
                p.cleared_by_install
                    .iter()
                    .map(move |a| (p, a, false))
                    .chain(p.still_exposed.iter().map(move |a| (p, a, true)))
            })
            .take(MAX_ADVISORY_CITATIONS)
            .map(|(p, advisory, pin_affected)| {
                let installed = p.installed.as_ref().map_or("", |c| c.version.as_str());
                let pin_note = if pin_affected { "so is" } else { "not" };
                EvidenceCitation {
                    source: "osv-advisory".to_string(),
                    title: truncate(&format!("{}: {}", advisory.id, advisory.summary), 160),
                    url: advisory.url.clone(),
                    freshness_days: 0.0,
                    relevance_note: truncate(
                        &format!(
                            "{} {installed} is affected; the pinned {} is {pin_note}",
                            p.package,
                            p.pinned_display()
                        ),
                        MAX_NOTE_BYTES,
                    ),
                }
            })
            .collect();
        out.extend(
            self.packages
                .iter()
                .take(MAX_PACKAGE_CITATIONS)
                .map(|p| self.installed_citation(p)),
        );
        let pins: Vec<String> = self
            .packages
            .iter()
            .take(MAX_PACKAGE_CITATIONS)
            .map(|p| format!("{} {}", p.package, p.pinned_display()))
            .collect();
        out.push(EvidenceCitation {
            source: "lockfile".to_string(),
            title: format!("{} pins {}", self.lockfile.file_name(), pins.join(", ")),
            url: None,
            freshness_days: 0.0,
            relevance_note: truncate(
                &Path::new(&self.project)
                    .join(self.lockfile.file_name())
                    .display()
                    .to_string(),
                MAX_NOTE_BYTES,
            ),
        });
        out
    }

    fn installed_citation(&self, p: &PackageDrift) -> EvidenceCitation {
        match &p.installed {
            Some(copy) => EvidenceCitation {
                source: "node_modules".to_string(),
                title: format!("node_modules holds {} {}", p.package, copy.version),
                url: None,
                freshness_days: 0.0,
                relevance_note: truncate(&copy.manifest.display().to_string(), MAX_NOTE_BYTES),
            },
            None => EvidenceCitation {
                source: "node_modules".to_string(),
                title: format!("{} is not installed", p.package),
                url: None,
                freshness_days: 0.0,
                relevance_note: truncate(
                    &format!(
                        "No node_modules/{} between {} and its repository root",
                        p.package, self.project
                    ),
                    MAX_NOTE_BYTES,
                ),
            },
        }
    }

    /// Informational only (doctrine rule 5): 4DA never runs the command.
    fn actions(&self, label: &str, opens_advisory: bool) -> Vec<Action> {
        let command = self.lockfile.install_command();
        let mut actions = vec![Action {
            action_id: "acknowledge".to_string(),
            label: format!("Run {command} in {label}"),
            description: format!(
                "4DA does not run it for you: run `{command}` in {}, and this row clears on \
                 the next refresh.",
                self.project
            ),
        }];
        if opens_advisory {
            actions.push(Action {
                action_id: "view_source".to_string(),
                label: "Open advisory".to_string(),
                description: "Open the advisory the running copy is exposed to".to_string(),
            });
        }
        actions.push(Action {
            action_id: "snooze_7d".to_string(),
            label: "Snooze 7 days".to_string(),
            description: "Hide this row for a week".to_string(),
        });
        actions.push(Action {
            action_id: "dismiss".to_string(),
            label: "Dismiss".to_string(),
            description: "Dismiss this row".to_string(),
        });
        actions
    }
}

/// Every live install-drift row, as the Preemption feed carries it.
pub fn detect(conn: &Connection) -> Vec<EvidenceItem> {
    projects(conn)
        .iter()
        .map(ProjectDrift::to_evidence_item)
        .collect()
}

/// The structured form — `preemption` projects the High rows into the
/// legacy alert feed the brief reads.
pub fn projects(conn: &Connection) -> Vec<ProjectDrift> {
    let liveness = ProjectLiveness::load(conn);
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    projects_with(conn, &liveness, &|path| {
        crate::project_inclusion::is_excluded_from_intelligence(path, &user_excluded)
    })
}

/// The core, with the two policy inputs injected so tests can drive it on a
/// temp directory (which the real inclusion policy excludes by design).
fn projects_with(
    conn: &Connection,
    liveness: &ProjectLiveness,
    is_excluded: &dyn Fn(&str) -> bool,
) -> Vec<ProjectDrift> {
    let mut drifted = Vec::new();
    for (project, pins) in direct_npm_pins(conn) {
        // The Preemption gates: an excluded project is not the user's to be
        // told about, and a dormant one is named once by
        // `collapse_dormant_alerts`, not here.
        if is_excluded(&project) || liveness.is_dormant(&project) {
            continue;
        }
        let dir = Path::new(&project);
        // Never installed is not drift: nothing runs to disagree with the lockfile.
        if !dir.join("node_modules").is_dir() {
            continue;
        }
        // No lockfile now means the stored pins are stale; nothing honest to say.
        let Some(lockfile) = Lockfile::detect(dir) else {
            continue;
        };
        let optional = optional_dependencies(dir);
        let mut packages: Vec<PackageDrift> = pins
            .into_iter()
            .filter_map(|(package, pin)| package_drift(conn, dir, package, pin, &optional))
            .collect();
        if packages.is_empty() {
            continue;
        }
        packages.sort_by(|a, b| {
            a.rank()
                .cmp(&b.rank())
                .then_with(|| a.package.cmp(&b.package))
        });
        drifted.push(ProjectDrift {
            project,
            lockfile,
            packages,
        });
    }
    drifted
}

/// What the lockfile pins for one direct dependency.
struct Pin {
    versions: BTreeSet<String>,
    dev_only: bool,
}

/// Direct npm pins per project, as the lockfile walk recorded them.
fn direct_npm_pins(conn: &Connection) -> BTreeMap<String, BTreeMap<String, Pin>> {
    let mut pins: BTreeMap<String, BTreeMap<String, Pin>> = BTreeMap::new();
    let Ok(mut stmt) = conn.prepare(
        "SELECT project_path, package_name, version, is_dev FROM dependency_instances
         WHERE ecosystem = 'npm' AND is_direct = 1",
    ) else {
        return pins;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
        ))
    }) else {
        return pins;
    };
    for (project, package, version, is_dev) in rows.filter_map(Result::ok) {
        let pin = pins
            .entry(project)
            .or_default()
            .entry(package)
            .or_insert_with(|| Pin {
                versions: BTreeSet::new(),
                dev_only: true,
            });
        pin.versions.insert(version);
        pin.dev_only &= is_dev;
    }
    pins
}

fn package_drift(
    conn: &Connection,
    dir: &Path,
    package: String,
    pin: Pin,
    optional: &BTreeSet<String>,
) -> Option<PackageDrift> {
    let installed = match resolve_installed(dir, &package) {
        Resolved::Installed(copy) if pin.versions.contains(&copy.version) => return None,
        Resolved::Installed(copy) => Some(copy),
        // A direct optional dependency that did not install (a platform-gated
        // binary) is npm working as designed.
        Resolved::Missing if optional.contains(&package) => return None,
        Resolved::Missing => None,
        Resolved::Unreadable => return None,
    };
    let exposure = installed
        .as_ref()
        .map(|copy| classify(conn, &package, &copy.version, &pin.versions))
        .unwrap_or_default();
    Some(PackageDrift {
        package,
        pinned: pin.versions,
        installed,
        cleared_by_install: exposure.cleared,
        still_exposed: exposure.still,
        pin_exposed: exposure.pin_exposed,
        dev_only: pin.dev_only,
    })
}

#[derive(Default)]
struct Exposure {
    cleared: Vec<AdvisoryRef>,
    still: Vec<AdvisoryRef>,
    pin_exposed: bool,
}

/// Sort every npm advisory for `package` by what the install would change.
fn classify(
    conn: &Connection,
    package: &str,
    installed: &str,
    pinned: &BTreeSet<String>,
) -> Exposure {
    let mut exposure = Exposure::default();
    for advisory in npm_advisories(conn, package) {
        let pin_hit = pinned
            .iter()
            .any(|version| confirmed_affected(&advisory.ranges, version));
        exposure.pin_exposed |= pin_hit;
        if !confirmed_affected(&advisory.ranges, installed) {
            continue;
        }
        let reference = AdvisoryRef {
            id: advisory.id,
            summary: advisory.summary,
            url: advisory.url,
        };
        if pin_hit {
            exposure.still.push(reference);
        } else {
            exposure.cleared.push(reference);
        }
    }
    exposure
}

/// Only a version-CONFIRMED range match is exposure: a range the matcher
/// cannot evaluate is evidence of nothing.
fn confirmed_affected(ranges: &Option<String>, version: &str) -> bool {
    crate::osv::matching::check_version_affected(Some(version), ranges) == (true, true)
}

struct NpmAdvisory {
    id: String,
    summary: String,
    url: Option<String>,
    ranges: Option<String>,
}

/// Ecosystem-scoped: an npm install is judged against npm advisories ONLY. A
/// name-only lookup would judge it against a same-named crates.io or PyPI
/// package's advisories — `knowledge_decay::still_vulnerable` is name-only,
/// which is why this module does not call it.
fn npm_advisories(conn: &Connection, package: &str) -> Vec<NpmAdvisory> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT advisory_id, summary, source_url, affected_ranges FROM osv_advisories
         WHERE lower(package_name) = lower(?1) AND ecosystem = 'npm' AND withdrawn_at IS NULL
         ORDER BY advisory_id",
    ) else {
        return Vec::new();
    };
    stmt.query_map([package], |row| {
        Ok(NpmAdvisory {
            id: row.get(0)?,
            summary: row.get(1)?,
            url: row.get(2)?,
            ranges: row.get(3)?,
        })
    })
    .map(|rows| rows.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

enum Resolved {
    Installed(InstalledCopy),
    Missing,
    Unreadable,
}

/// Resolve `package` from `project_dir` the way Node does: `node_modules/<name>`
/// in the project, then in each parent — where hoisted workspaces install —
/// stopping after the repository root (the first directory holding `.git`),
/// at most [`MAX_PARENT_LEVELS`] levels up.
fn resolve_installed(project_dir: &Path, package: &str) -> Resolved {
    let Some(relative) = package_path(package) else {
        tracing::debug!(target: "4da::install_drift", package, "not a resolvable npm package name");
        return Resolved::Unreadable;
    };
    let mut current = Some(project_dir);
    for _ in 0..=MAX_PARENT_LEVELS {
        let Some(dir) = current else {
            break;
        };
        let manifest = dir
            .join("node_modules")
            .join(&relative)
            .join("package.json");
        if manifest.is_file() {
            let version = read_json(&manifest)
                .and_then(|json| json.get("version")?.as_str().map(|v| v.trim().to_string()))
                .filter(|v| !v.is_empty());
            return match version {
                Some(version) => Resolved::Installed(InstalledCopy { version, manifest }),
                None => {
                    tracing::debug!(
                        target: "4da::install_drift",
                        manifest = %manifest.display(),
                        "installed package.json unreadable or versionless — skipped"
                    );
                    Resolved::Unreadable
                }
            };
        }
        if dir.join(".git").exists() {
            break;
        }
        current = dir.parent();
    }
    Resolved::Missing
}

/// `name` or `@scope/name` as path components. A name that could climb out
/// of `node_modules` is refused.
fn package_path(package: &str) -> Option<PathBuf> {
    let segments: Vec<&str> = package.split('/').collect();
    let well_formed = match segments.as_slice() {
        [name] => !name.starts_with('@'),
        [scope, _] => scope.starts_with('@') && scope.len() > 1,
        _ => false,
    };
    let safe = segments
        .iter()
        .all(|s| !s.is_empty() && *s != "." && *s != ".." && !s.contains(['\\', ':']));
    (well_formed && safe).then(|| segments.iter().collect())
}

/// Names the project's package.json lists under `optionalDependencies`.
fn optional_dependencies(dir: &Path) -> BTreeSet<String> {
    read_json(&dir.join("package.json"))
        .and_then(|json| {
            json.get("optionalDependencies")?
                .as_object()
                .map(|deps| deps.keys().cloned().collect())
        })
        .unwrap_or_default()
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn advisory_ids(advisories: &[AdvisoryRef]) -> String {
    let ids: Vec<&str> = advisories.iter().take(3).map(|a| a.id.as_str()).collect();
    let more = advisories.len().saturating_sub(ids.len());
    if more > 0 {
        format!("{} and {more} more", ids.join(", "))
    } else {
        ids.join(", ")
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
#[path = "install_drift_tests.rs"]
mod tests;
