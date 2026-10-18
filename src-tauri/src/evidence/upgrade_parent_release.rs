// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The words for a parent release read from the registry (audit H1,
//! `osv::parent_release`): which release of the parent admits a clean copy,
//! the command that moves the dependency the project declares, and the
//! refresh that must follow when an install would keep the vulnerable copy.
//! Shared by the Upgrade Plan, Preemption and the Brief through
//! [`super::parent_move`], so the three name the same release.
//!
//! Every version follows its own package's name (the Brief's version check,
//! `briefing_groundedness::check_factual_claims`, reads the nearest preceding
//! package).

use crate::osv::parent_hint::{ParentLink, ParentRelease, ParentStanding, Through};

/// "express 4.22.0 is the smallest express release whose requirement
/// "~0.1.12" admits path-to-regexp 0.1.13 (a fresh resolve picks
/// path-to-regexp 0.1.13), and no advisory of express affects it".
fn release_clause(package: &str, target: &str, parent: &str, r: &ParentRelease) -> String {
    let own = if r.own_checked {
        format!(", and no advisory of {parent} affects it")
    } else {
        String::new()
    };
    let resolves = r.resolves_to.as_deref().map_or_else(String::new, |v| {
        format!(" (a fresh resolve picks {package} {v})")
    });
    let req = r.requirement.as_deref().unwrap_or("");
    let kind = if r.peer {
        "peer requirement"
    } else {
        "requirement"
    };
    let peer = if r.peer {
        " (npm 7+ installs a missing peer)"
    } else {
        ""
    };
    match (&r.smallest, &r.requirement) {
        (Some(smallest), _) => {
            let its = match (&r.requirement, &r.resolves_to) {
                (Some(_), Some(v)) => format!(" (its {kind} \"{req}\" resolves to {package} {v})"),
                (Some(_), None) => format!(" (its {kind} \"{req}\")"),
                (None, _) => format!(" (it no longer depends on {package})"),
            };
            format!(
                "{parent} {smallest} is the smallest {parent} release that admits {package} \
                 {target}, and {parent} {to}, the release {parent}'s own upgrade moves to, admits \
                 it too{its}, so one bump serves both{own}",
                to = r.to
            )
        }
        (None, None) => format!(
            "{parent} {to} is the smallest {parent} release that no longer depends on {package}{own}",
            to = r.to
        ),
        (None, Some(_)) => format!(
            "{parent} {to} is the smallest {parent} release whose {kind} \"{req}\" admits \
             {package} {target}{peer}{resolves}{own}",
            to = r.to
        ),
    }
}

fn major(r: &ParentRelease) -> &'static str {
    if r.breaking {
        " (a major upgrade)"
    } else {
        ""
    }
}

fn command(r: &ParentRelease) -> String {
    r.command
        .as_deref()
        .map_or_else(String::new, |c| format!(" (`{c}`)"))
}

/// "; path-to-regexp 0.1.12 still satisfies "~0.1.12", and an install keeps
/// a copy that satisfies it, so then run `npm update path-to-regexp`".
fn keeps(package: &str, r: &ParentRelease) -> String {
    let Some(kept) = r.kept.as_deref() else {
        return String::new();
    };
    let then = r.refresh.as_deref().map_or_else(
        || format!("so a lockfile refresh of {package} must follow"),
        |c| format!("so then run `{c}`"),
    );
    match r.requirement.as_deref() {
        Some(req) => format!(
            "; {package} {kept} still satisfies \"{req}\", and an install keeps a copy that \
             satisfies it, {then}"
        ),
        None => format!("; another parent still holds {package} {kept}, {then}"),
    }
}

/// The move when the registry named `parent`'s release: upgrade the parent
/// itself, every declared dependency it arrives through, or (when none can be
/// tied to a declaration) name the release and no command. `force` is the
/// override clause (", or force ... with ..."), offered wherever a lever
/// cannot move.
pub(super) fn admits_move(
    package: &str,
    target: &str,
    parent: &str,
    r: &ParentRelease,
    through: Option<&Through>,
    force: &str,
) -> String {
    let why = release_clause(package, target, parent, r);
    let to = &r.to;
    let Some(t) = through else {
        if let Some(reason) = r.lever_unknown.as_deref() {
            return format!(
                "{why}. Which of this project's dependencies brings {parent} in could not be \
                 worked out ({reason}), so no command is named: bump the one that does to a \
                 release that brings in {parent} >= {to}{}{force}",
                keeps(package, r)
            );
        }
        return format!(
            "{why}: upgrade {parent} to {to}{}{}{}",
            major(r),
            command(r),
            keeps(package, r)
        );
    };
    let levers: Vec<String> = t.all().map(|l| lever_move(parent, to, l, force)).collect();
    let more = match t.more {
        0 => String::new(),
        1 => "; 1 more declared dependency brings it in and must move the same way".to_string(),
        n => format!("; {n} more declared dependencies bring it in and must move the same way"),
    };
    format!(
        "{why}. {}: {}{more}{}",
        arrives(parent, t),
        levers.join("; "),
        keeps(package, r)
    )
}

/// "express arrives through a 1.0.0 and b 1.0.0, direct dependencies, each
/// of which must move".
fn arrives(parent: &str, t: &Through) -> String {
    let names: Vec<String> = t
        .all()
        .map(|l| format!("{} {}", l.name, l.version))
        .collect();
    let total = names.len() + t.more;
    let list = match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.concat(),
    };
    let more = if t.more > 0 {
        format!(" (and {} more)", t.more)
    } else {
        String::new()
    };
    if total == 1 {
        format!("{parent} arrives through {list}, a direct dependency")
    } else {
        format!(
            "{parent} arrives through {list}{more}, direct dependencies, each of which must move"
        )
    }
}

/// What one declared dependency the parent arrives through can do.
fn lever_move(parent: &str, to: &str, l: &Through, force: &str) -> String {
    let lever = &l.name;
    match &l.standing {
        ParentStanding::Admits(h) => {
            let brings = h.brings.as_deref().unwrap_or(to);
            let floor = h
                .smallest
                .as_deref()
                .map_or_else(String::new, |s| format!(" (>= {parent} {s})"));
            format!(
                "{lever} {} is the smallest {lever} release that brings in {parent} \
                 {brings}{floor}: upgrade {lever} to {}{}{}{}",
                h.to,
                h.to,
                major(h),
                command(h),
                keeps(parent, h)
            )
        }
        ParentStanding::NoneAdmits { .. } => format!(
            "no {lever} release up to its newest brings in a {parent} that admits it: replace \
             {lever}{force}"
        ),
        ParentStanding::Deprecated | ParentStanding::Newest { deprecated: true } => {
            format!("{lever} is deprecated: replace {lever}{force}")
        }
        ParentStanding::Newest { deprecated: false } => format!(
            "{lever} {} is already its latest release: replace {lever}{force}",
            l.version
        ),
        ParentStanding::Newer { latest } => format!(
            "upgrade {lever} to a release that brings in {parent} >= {to} ({lever} {latest} is \
             the newest release 4DA has seen; check that it does)"
        ),
        ParentStanding::Unknown => format!(
            "upgrade {lever} to a release that brings in {parent} >= {to}, and check that it does"
        ),
    }
}

/// The registry fact behind a named release, as a `dependency-path`
/// citation: "express 4.22.0 -> path-to-regexp ~0.1.12".
pub(super) fn release_citation(package: &str, l: &ParentLink) -> Option<(String, String)> {
    let ParentStanding::Admits(r) = &l.standing else {
        return None;
    };
    let req = r.requirement.as_deref().unwrap_or("nothing");
    let title = format!("{} {} -> {package} {req}", l.parent, r.to);
    let resolves = r.resolves_to.as_deref().map_or_else(String::new, |v| {
        format!(", which resolves to {package} {v}")
    });
    let note = format!(
        "Registry: {} {} requires {package} {req}{resolves}; no advisory 4DA holds with a \
         published fix affects that copy",
        l.parent, r.to
    );
    Some((title, note))
}
