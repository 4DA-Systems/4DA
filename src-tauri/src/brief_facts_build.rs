// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Brief facts, DB-backed builders: the security, upgrade and worth-knowing
//! lanes read from the Preemption feed, the lockfile graph, the registry rows
//! and the judged feed. Split from `brief_facts.rs` (file-size gate); the
//! types and the pure rules they apply live there.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::db::Database;
use crate::osv::fix_path::{installed_parent_requirements, Manager, Requirement};
use crate::preemption::{AlertUrgency, PreemptionAlert};
use crate::scoring::release_grade::{self, ReleaseClass};

use super::*;

/// Build every fact the brief is written from. Best-effort: a failed lane
/// yields an empty lane, never an error that costs the user the whole brief.
pub(crate) fn build_brief_facts(db: &Database) -> BriefFacts {
    let novelty = Novelty::load(db);
    let today = local_today();
    let mut labels = LabelCache::default();
    let liveness = crate::open_db_connection()
        .map(|conn| crate::evidence::ProjectLiveness::load(&conn))
        .unwrap_or_default();

    let (security, also_open) = build_security_facts(db, &novelty, &liveness, &mut labels);
    let upgrades = build_upgrade_facts(db, &novelty, &liveness, &mut labels);
    let worth_knowing = build_worth_knowing(db, &novelty, &today);
    let fingerprint = fingerprint(&security, &also_open, &upgrades);
    // A failed read keeps the old behaviour (dependencies assumed known)
    // rather than telling a scanned user to scan.
    let no_dependencies_known = crate::open_db_connection()
        .map(|conn| crate::knowledge_decay::known_dependency_count(&conn) == 0)
        .unwrap_or(false);
    BriefFacts {
        security,
        also_open,
        upgrades,
        worth_knowing,
        fingerprint,
        no_dependencies_known,
    }
}

fn norm_path(p: &str) -> String {
    p.to_lowercase()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

/// Repository-aware labels, memoised per build (each probe walks up to the
/// nearest `.git`).
#[derive(Default)]
pub(crate) struct LabelCache {
    map: HashMap<String, String>,
}

impl LabelCache {
    pub(crate) fn label(&mut self, path: &str) -> String {
        let key = norm_path(path);
        if let Some(l) = self.map.get(&key) {
            return l.clone();
        }
        let label = repo_label(path).unwrap_or_else(|| crate::privacy_egress::project_label(path));
        self.map.insert(key, label.clone());
        label
    }
}

fn repo_label(path: &str) -> Option<String> {
    let start = std::path::Path::new(path);
    let mut dir = Some(start);
    for _ in 0..8 {
        let d = dir?;
        if d.join(".git").exists() {
            let repo = d.file_name()?.to_string_lossy().into_owned();
            let rel: Vec<String> = start
                .strip_prefix(d)
                .ok()?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            let rel_refs: Vec<&str> = rel.iter().map(String::as_str).collect();
            return Some(label_from(&repo, &rel_refs));
        }
        dir = d.parent();
    }
    None
}

fn urgency_rank(u: &AlertUrgency) -> u8 {
    match u {
        AlertUrgency::Critical => 0,
        AlertUrgency::High => 1,
        AlertUrgency::Medium => 2,
        AlertUrgency::Watch => 3,
    }
}

fn tier_rank(t: &str) -> u8 {
    match t {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

fn build_security_facts(
    db: &Database,
    novelty: &Novelty,
    liveness: &crate::evidence::ProjectLiveness,
    labels: &mut LabelCache,
) -> (Vec<SecurityFact>, Vec<SecurityFact>) {
    let feed = match crate::preemption::get_preemption_feed() {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(target: "4da::briefing", error = %e, "brief facts: preemption feed unavailable");
            return (Vec::new(), Vec::new());
        }
    };
    let (matched, not_compiled) =
        crate::osv::matching::get_matched_advisories_with_not_compiled(db).unwrap_or_default();
    let conn = crate::open_db_connection().ok();
    let ctx = FactCtx {
        db,
        matched: &matched,
        not_compiled: &not_compiled,
        conn: conn.as_ref(),
        novelty,
        liveness,
    };

    let mut act = Vec::new();
    let mut also = Vec::new();
    for alert in feed
        .alerts
        .iter()
        .filter(|a| a.osv_verified && !a.platform_inactive)
    {
        let Some(fact) = security_fact(alert, &ctx, labels) else {
            continue;
        };
        // A scratch tree its own repository gitignores (victauri-gauntlet)
        // stays on the Preemption tab; the brief does not nag about it.
        let all_scratch = fact.sites.iter().all(|s| s.scratch);
        match fact.urgency {
            _ if all_scratch => {}
            AlertUrgency::Critical | AlertUrgency::High => act.push(fact),
            AlertUrgency::Watch => {}
            AlertUrgency::Medium => also.push(fact),
        }
    }
    act.sort_by(|a, b| {
        urgency_rank(&a.urgency)
            .cmp(&urgency_rank(&b.urgency))
            .then_with(|| a.package.cmp(&b.package))
    });
    also.sort_by(|a, b| {
        urgency_rank(&a.urgency)
            .cmp(&urgency_rank(&b.urgency))
            .then_with(|| a.package.cmp(&b.package))
    });
    (act, also)
}

/// What every security fact of one build reads.
struct FactCtx<'a> {
    db: &'a Database,
    matched: &'a [crate::osv::types::MatchedAdvisory],
    not_compiled: &'a [crate::osv::types::NotCompiledMatch],
    conn: Option<&'a rusqlite::Connection>,
    novelty: &'a Novelty,
    liveness: &'a crate::evidence::ProjectLiveness,
}

fn security_fact(
    alert: &PreemptionAlert,
    ctx: &FactCtx<'_>,
    labels: &mut LabelCache,
) -> Option<SecurityFact> {
    let FactCtx {
        matched,
        conn,
        novelty,
        liveness,
        ..
    } = *ctx;
    let package = alert.affected_dependencies.first()?.clone();
    let projects: Vec<String> = alert
        .affected_projects
        .iter()
        .map(|p| norm_path(p))
        .collect();
    let pkg_key = canonical_package(&package);
    let group: Vec<&crate::osv::types::MatchedAdvisory> = matched
        .iter()
        .filter(|m| m.is_version_confirmed && canonical_package(&m.package_name) == pkg_key)
        .filter(|m| {
            projects.is_empty()
                || m.project_paths
                    .iter()
                    .any(|p| projects.contains(&norm_path(p)))
        })
        .collect();
    let ecosystem = group
        .first()
        .map(|m| m.ecosystem.clone())
        .unwrap_or_else(|| "npm".to_string());
    let clusters = crate::osv::identity::cluster_by_vulnerability(&group);
    let worst_tier = clusters
        .iter()
        .filter_map(|c| crate::osv::identity::cluster_severity_tier(c))
        .min_by_key(|t| tier_rank(t))
        .map(str::to_string);
    let advisory_ids: Vec<String> = clusters.iter().map(|c| c[0].advisory_id.clone()).collect();
    let is_drift = alert.id.starts_with("install-drift:");
    // "rmcp@1.7.0: 3 known vulnerabilities" says nothing a reader can weigh;
    // the worst advisory's own summary says what the bug IS.
    let worst_summary = clusters
        .iter()
        .min_by_key(|c| crate::osv::identity::cluster_severity_tier(c).map_or(4, tier_rank))
        .map(|c| c[0].summary.trim().to_string())
        .filter(|s| !s.is_empty());
    let title = match worst_summary {
        Some(summary) if alert.title.contains("known vulnerabilit") => {
            if clusters.len() > 1 {
                format!("{summary} (worst of {})", clusters.len())
            } else {
                summary
            }
        }
        _ => alert.title.trim().to_string(),
    };

    // Newer parents other projects here already resolved past the advisories
    // (the same read Preemption's upgrade plan names them from).
    let parent_links = crate::osv::exposure::canonical(&ecosystem)
        .map(|eco| {
            let lines = crate::osv::fix_target::line_targets(&group, &projects);
            crate::osv::parent_hint::load_parent_links(ctx.db, eco, &package, &lines)
        })
        .unwrap_or_default();

    let mut sites = Vec::new();
    for project in &alert.affected_projects {
        let pnorm = norm_path(project);
        // The confirmed instance in this project, when the matcher has one.
        let instance = group
            .iter()
            .flat_map(|m| m.dependency_instances.iter())
            .find(|d| d.is_version_confirmed && norm_path(&d.project_path) == pnorm);
        let installed = instance
            .and_then(|d| d.installed_version.clone())
            .or_else(|| alert.installed_version.clone());
        let is_direct = instance.map(|d| d.is_direct).or(alert.is_direct);
        let dev_only = instance.map(|d| d.is_dev).or(alert.is_dev).unwrap_or(false);
        let fix = alert.fixed_version.as_deref();
        let fix_path = if is_drift {
            match fix {
                Some(to) => FixPath::Reinstall { to: to.to_string() },
                None => FixPath::NoFix,
            }
        } else {
            let parent = if is_direct == Some(false) {
                conn.and_then(|c| {
                    direct_parent(c, project, &ecosystem, &package, installed.as_deref(), fix)
                })
            } else {
                None
            };
            let path = fix_path(installed.as_deref(), fix, is_direct, parent.as_ref());
            let path =
                with_refresh_command(path, project, &ecosystem, &package, installed.as_deref());
            with_proven_parent(path, &parent_links, &pnorm, labels)
        };
        sites.push(SecuritySite {
            label: labels.label(project),
            installed,
            dev_only,
            scratch: liveness.is_scratch(project),
            dormant_days: liveness
                .dormant_days(project)
                .filter(|d| crate::ace::dormancy::is_dormant_days(*d)),
            fix_path,
        });
    }
    sites.sort_by(|a, b| a.label.cmp(&b.label));
    sites.dedup_by(|a, b| a.label == b.label && a.installed == b.installed);

    let site_labels: BTreeSet<&str> = sites.iter().map(|s| s.label.as_str()).collect();
    let key = format!(
        "{}:{}:{}{}",
        ecosystem.to_lowercase(),
        pkg_key,
        site_labels.into_iter().collect::<Vec<_>>().join(","),
        if is_drift { ":drift" } else { "" }
    );
    let first_seen = conn.and_then(|c| {
        crate::digest_commands::first_seen_for_ids(c, &advisory_ids).map(|(date, _)| date)
    });
    let not_compiled = not_compiled_notes(ctx.not_compiled, &group, &pkg_key, &projects);
    let mut fact = SecurityFact {
        key,
        package,
        ecosystem,
        urgency: alert.urgency.clone(),
        worst_tier,
        advisory_count: clusters.len().max(1),
        advisory_ids,
        title,
        sites,
        not_compiled,
        first_seen,
        status: FactStatus::New,
    };
    fact.status = novelty.status(&fact.key, &security_signature(&fact), &local_today());
    Some(fact)
}

/// Name the project tool's refresh command on a refresh path (`osv::fix_path`).
fn with_refresh_command(
    path: FixPath,
    project: &str,
    ecosystem: &str,
    package: &str,
    installed: Option<&str>,
) -> FixPath {
    let FixPath::Refresh {
        to,
        inferred,
        command: None,
    } = path
    else {
        return path;
    };
    let command = crate::osv::exposure::canonical(ecosystem)
        .and_then(|eco| Manager::detect(std::path::Path::new(project), eco))
        .zip(installed)
        .and_then(|(m, installed)| m.refresh_command(package, installed, &to));
    FixPath::Refresh {
        to,
        inferred,
        command,
    }
}

/// Attach the lockfile proof of a parent release that clears the package, if
/// another project here runs one. Only a link for the SAME parent at the SAME
/// installed version counts: the brief names the direct dependency at the top
/// of the chain, and a hint about a different link would name the wrong one.
fn with_proven_parent(
    path: FixPath,
    links: &[crate::osv::parent_hint::ParentLink],
    project: &str,
    labels: &mut LabelCache,
) -> FixPath {
    let FixPath::Parent {
        parent,
        parent_version,
        to,
        by_requirement,
        proven: None,
    } = path
    else {
        return path;
    };
    let proven = links
        .iter()
        .filter(|l| {
            l.project == project
                && l.parent.eq_ignore_ascii_case(&parent)
                && l.parent_version == parent_version
        })
        .find_map(|l| l.resolutions.first())
        .map(|r| ProvenParent {
            parent_version: r.parent_version.clone(),
            child_version: r.child_version.clone(),
            label: labels.label(&r.project),
        });
    FixPath::Parent {
        parent,
        parent_version,
        to,
        by_requirement,
        proven,
    }
}

/// The package's advisories that NONE of the fact's projects compile. One the
/// matcher kept for some project stays counted, so it is not noted here.
fn not_compiled_notes(
    excluded: &[crate::osv::types::NotCompiledMatch],
    group: &[&crate::osv::types::MatchedAdvisory],
    pkg_key: &str,
    projects: &[String],
) -> Vec<NotCompiledNote> {
    let mut notes: Vec<NotCompiledNote> = Vec::new();
    for n in excluded {
        let in_scope = canonical_package(&n.package_name) == pkg_key
            && projects.contains(&norm_path(&n.project_path));
        let still_counted = group.iter().any(|m| m.advisory_id == n.advisory_id);
        if in_scope && !still_counted && !notes.iter().any(|x| x.advisory_id == n.advisory_id) {
            notes.push(NotCompiledNote {
                advisory_id: n.advisory_id.clone(),
                summary: n.summary.trim().to_string(),
            });
        }
    }
    notes.sort_by(|a, b| a.advisory_id.cmp(&b.advisory_id));
    notes
}

/// Walk `dependency_edges` up from `package` in `project` to the DIRECT
/// dependency that pulls it in, using only edges whose parent version is the
/// one installed now (the table keeps stale rows from earlier scans).
fn direct_parent(
    conn: &rusqlite::Connection,
    project: &str,
    ecosystem: &str,
    package: &str,
    installed_version: Option<&str>,
    target: Option<&str>,
) -> Option<ParentLink> {
    let edge_eco = match ecosystem.to_lowercase().as_str() {
        "crates.io" | "rust" => "rust",
        "npm" | "javascript" => "javascript",
        _ => return None,
    };
    let inst_eco = if edge_eco == "rust" {
        "crates.io"
    } else {
        "npm"
    };
    let pnorm = norm_path(project);

    // Every installed version per package: a project can carry two copies
    // (thiserror 1.x beside 2.x), and only edges from a copy installed now
    // count — the edge table keeps rows from earlier scans.
    let installed: HashMap<String, Vec<(String, bool)>> = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT package_name, version, is_direct FROM dependency_instances
                 WHERE LOWER(REPLACE(project_path, '\\', '/')) = ?1 AND ecosystem = ?2",
            )
            .ok()?;
        let rows = stmt
            .query_map(rusqlite::params![pnorm, inst_eco], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)? != 0,
                ))
            })
            .ok()?;
        let mut map: HashMap<String, Vec<(String, bool)>> = HashMap::new();
        for (name, version, direct) in rows.flatten() {
            map.entry(name).or_default().push((version, direct));
        }
        map
    };
    let mut stmt = conn
        .prepare_cached(
            "SELECT parent_package, parent_version, child_version FROM dependency_edges
             WHERE LOWER(REPLACE(project_path, '\\', '/')) = ?1 AND ecosystem = ?2
               AND child_package = ?3",
        )
        .ok()?;
    let mut edges_of = |child: &str| -> Vec<(String, Option<String>, Option<String>)> {
        stmt.query_map(rusqlite::params![pnorm, edge_eco, child], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    };

    // Only npm's package-lock records the parent's REQUIREMENT on the child.
    // Cargo.lock and pnpm-lock record the resolved version, which read as a
    // requirement would be an exact pin and send every patch fix to the
    // parent (2026-10-02 review: 2,153 Cargo rows, 1,765 of 1,767 pnpm rows);
    // for those the parent's installed manifest is read instead
    // (`osv::fix_path`, the rule the plan applies too).
    let manager = Manager::detect(std::path::Path::new(project), inst_eco);
    let requirement_recorded = manager.is_some_and(Manager::records_requirements);

    // Each frontier entry carries the requirements of the depth-0 edge it
    // descends from, so the reported requirement is the one on THIS chain.
    let mut frontier: Vec<(String, Vec<Requirement>)> = vec![(package.to_string(), Vec::new())];
    let mut seen: HashSet<String> = HashSet::new();
    for depth in 0..MAX_PARENT_DEPTH {
        let mut next = Vec::new();
        for (child, chain_req) in &frontier {
            let mut hops: Vec<(String, String, bool, Vec<Requirement>)> = Vec::new();
            for (parent, parent_version, child_req) in edges_of(child) {
                if parent == "__root__" || seen.contains(&parent) {
                    continue;
                }
                // Only an edge from a copy of the parent installed now. The
                // seen-mark comes AFTER this filter: marking first let a stale
                // row hide the current one (rmcp in 4da/src-tauri resolved to
                // no parent because the 0.8.5 row came before the 0.9.0 one).
                let Some((current, direct)) = installed.get(&parent).and_then(|copies| {
                    copies
                        .iter()
                        .find(|(v, _)| parent_version.as_deref().is_none_or(|pv| pv == v))
                }) else {
                    continue;
                };
                let req = if depth > 0 {
                    chain_req.clone()
                } else if requirement_recorded {
                    let req: Vec<Requirement> = child_req
                        .filter(|r| !r.trim().is_empty())
                        .map(|r| vec![Requirement::npm(&r)])
                        .unwrap_or_default();
                    // A recorded requirement that excludes the installed copy
                    // belongs to ANOTHER copy of the package.
                    let other_copy = installed_version
                        .is_some_and(|v| req.iter().any(|r| r.admits(v) == Some(false)));
                    if other_copy {
                        continue;
                    }
                    req
                } else {
                    manager.map_or_else(Vec::new, |m| {
                        installed_parent_requirements(
                            m,
                            std::path::Path::new(project),
                            &parent,
                            current,
                            package,
                        )
                    })
                };
                hops.push((parent, current.clone(), *direct, req));
            }
            // A copy any of whose parents excludes the fix is not fixed by a
            // refresh (traefik/webui's js-yaml 3.13.1: three parents take
            // ^3.13, mocha pins 3.13.1) — that parent's chain is the one to
            // report, so it is walked first.
            if depth == 0 {
                hops.sort_by_key(|(.., req)| {
                    !target.is_some_and(|t| req.iter().any(|r| r.admits(t) == Some(false)))
                });
            }
            for (parent, current, direct, req) in hops {
                if !seen.insert(parent.clone()) {
                    continue;
                }
                if direct {
                    return Some(ParentLink {
                        direct: parent,
                        direct_version: current,
                        requirements: req,
                    });
                }
                next.push((parent, req));
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    None
}

/// One key per registry family, so `crates_io` and `crates` rows of the same
/// crate collapse into one fact.
fn registry_family(source_type: &str) -> &str {
    match source_type {
        "crates_io" | "crates" => "rust",
        "npm_registry" | "npm" => "javascript",
        "go_modules" | "go" => "go",
        other => other,
    }
}

/// Detected project names = the packages the user publishes from their own
/// workspaces.
pub(super) fn own_package_names() -> HashSet<String> {
    let Ok(conn) = crate::open_db_connection() else {
        return HashSet::new();
    };
    let Ok(mut stmt) = conn.prepare("SELECT name FROM detected_projects") else {
        return HashSet::new();
    };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map(|rows| rows.flatten().map(|n| canonical_package(&n)).collect())
        .unwrap_or_default()
}

fn build_upgrade_facts(
    db: &Database,
    novelty: &Novelty,
    liveness: &crate::evidence::ProjectLiveness,
    labels: &mut LabelCache,
) -> Vec<UpgradeFact> {
    let own = own_package_names();
    let rows: Vec<(i64, String, String, String, Option<String>, Option<String>)> = {
        let Ok(conn) = crate::open_db_connection() else {
            return Vec::new();
        };
        let placeholders = crate::dep_linker::REGISTRY_SOURCE_TYPES
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, source_type, title, COALESCE(content, ''), url,
                    COALESCE(published_at, created_at)
             FROM source_items
             WHERE source_type IN ({placeholders})
               AND COALESCE(published_at, created_at) >= datetime('now', ?1)
             ORDER BY id DESC LIMIT 3000"
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            return Vec::new();
        };
        let window = format!("-{UPGRADE_WINDOW_DAYS} days");
        stmt.query_map(rusqlite::params![window], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map(|rs| rs.flatten().collect())
        .unwrap_or_default()
    };

    // (lang, package) -> best grade seen (highest announced version).
    let mut best: BTreeMap<
        (String, String),
        (
            release_grade::ReleaseGrade,
            i64,
            Option<String>,
            Option<String>,
            String,
        ),
    > = BTreeMap::new();
    for (id, source_type, title, content, url, published) in rows {
        let Some(grade) = release_grade::grade_registry_release(db, &source_type, &title, &content)
        else {
            continue;
        };
        if !matches!(
            grade.class(),
            Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
        ) {
            continue;
        }
        if is_own_package(&grade.package, &own) {
            continue;
        }
        let lang = registry_family(&source_type).to_string();
        let key = (lang, canonical_package(&grade.package));
        let replace = best
            .get(&key)
            .is_none_or(|(g, ..)| grade.announced > g.announced);
        if replace {
            best.insert(key, (grade, id, url, published, source_type));
        }
    }

    let mut facts: Vec<UpgradeFact> = Vec::new();
    for ((_, pkg_key), (grade, id, url, published, source_type)) in best {
        let concerned = grade.concerned_pins();
        let active: Vec<_> = concerned
            .iter()
            .filter(|p| {
                !liveness.is_scratch(&p.project_path) && !liveness.is_dormant(&p.project_path)
            })
            .collect();
        if active.is_empty() {
            continue;
        }
        let yanked = matches!(grade.class(), Some(ReleaseClass::Yanked));
        let dev_only = active.iter().all(|p| p.is_dev);
        let announced = grade.announced.to_string();
        let lowest = active
            .iter()
            .map(|p| &p.installed)
            .min()
            .map(ToString::to_string);
        let mut sites: Vec<UpgradeSite> = active
            .iter()
            .map(|p| UpgradeSite {
                label: labels.label(&p.project_path),
                installed: p.installed.to_string(),
            })
            .collect();
        sites.sort_by(|a, b| a.label.cmp(&b.label));
        sites.dedup_by(|a, b| a.label == b.label);
        let ecosystem = if matches!(source_type.as_str(), "crates_io" | "crates") {
            "crates.io"
        } else if matches!(source_type.as_str(), "npm_registry" | "npm") {
            "npm"
        } else {
            source_type.as_str()
        }
        .to_string();
        let key = format!("{ecosystem}:{pkg_key}");
        let status = novelty.status(
            &key,
            &crate::brief_cadence::upgrade_signature_of(&announced, yanked),
            &local_today(),
        );
        facts.push(UpgradeFact {
            key,
            package: grade.package.clone(),
            ecosystem,
            majors_behind: lowest
                .as_deref()
                .map_or(0, |l| majors_behind(l, &announced)),
            pre_one: grade.announced.major == 0,
            announced,
            published: published.map(|p| p.chars().take(10).collect()),
            yanked,
            dev_only,
            sites,
            item_id: id,
            url,
            status,
        });
    }
    // Yanked first, then runtime before tooling, the widest gap, the newest
    // release. Never by novelty: reporting a brief must not change WHICH
    // facts are selected, or the next auto trigger sees a different
    // fingerprint and regenerates (live 2026-10-02: the second run of the
    // same day swapped five of eight upgrades and rewrote the brief).
    facts.sort_by(|a, b| {
        b.yanked
            .cmp(&a.yanked)
            .then(a.dev_only.cmp(&b.dev_only))
            .then(b.majors_behind.cmp(&a.majors_behind))
            .then(b.published.cmp(&a.published))
            .then(a.package.cmp(&b.package))
    });
    facts.truncate(MAX_UPGRADES);
    facts
}

fn build_worth_knowing(
    db: &Database,
    novelty: &Novelty,
    today: &str,
) -> Vec<WorthKnowingCandidate> {
    let _ = db;
    let Ok(conn) = crate::open_db_connection() else {
        return Vec::new();
    };
    // AD-054 rule 3: worth knowing reads enabled interests only (social and
    // editorial reading is opt-in; registries and advisories feed the stack
    // sections, not this one). None enabled: the section is empty.
    let interests = crate::brief_interests::sql_in_list(
        &crate::brief_interests::enabled_interest_source_types(&conn),
    );
    if interests.is_empty() {
        return Vec::new();
    }
    let user_lang = crate::i18n::get_user_language();
    // Order by the judge's latest verdict, then the feed rank. Live
    // 2026-10-02 the feed rank put a generic "52 utilities" post first and
    // left "Announcing Tauri 2.12" (judge 0.92) and a Stripe webhook bug for
    // a Stripe project (0.90) further down. An item the judge scored below
    // its own feed bar (0.5) is not a candidate, whatever its rank.
    let sql = format!(
        "SELECT id, title, url, source_type, COALESCE(content, ''), eff FROM (
             SELECT s.id, s.title, s.url, s.source_type, s.content,
                    COALESCE(s.published_at, s.created_at) AS eff,
                    COALESCE(s.rank_score, s.relevance_score, 0) AS rank,
                    (SELECT j.relevance_score FROM llm_judgments j
                      WHERE j.source_item_id = s.id
                      ORDER BY j.judged_at DESC, j.id DESC LIMIT 1) AS judge
             FROM source_items s
             WHERE s.feed_relevant = 1
               AND s.source_type IN ({interests})
               AND TRIM(s.title) <> ''
               AND COALESCE(s.detected_lang, 'en') = ?1
               AND COALESCE(s.published_at, s.created_at) >= datetime('now', ?2)
         )
         WHERE judge IS NULL OR judge >= {bar}
         ORDER BY COALESCE(judge, rank) DESC, rank DESC, id DESC
         LIMIT 60",
        bar = WORTH_KNOWING_JUDGE_BAR
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let window = format!("-{WORTH_KNOWING_WINDOW_DAYS} days");
    let rows: Vec<(i64, String, Option<String>, String, String, String)> = stmt
        .query_map(rusqlite::params![user_lang, window], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map(|rs| rs.flatten().collect())
        .unwrap_or_default();

    let mut seen_titles: HashSet<String> = HashSet::new();
    rows.into_iter()
        .filter(|(id, ..)| !novelty.featured_before(*id, today))
        .filter(|(_, title, ..)| seen_titles.insert(title.trim().to_lowercase()))
        .take(WORTH_KNOWING_CANDIDATES)
        .map(
            |(id, title, url, source_type, body, published)| WorthKnowingCandidate {
                id,
                title: title.trim().to_string(),
                url,
                source_type,
                published: published.chars().take(10).collect(),
                excerpt: excerpt(&body, EXCERPT_CHARS),
            },
        )
        .collect()
}
