// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Whether an npm requirement admits a version: the one reader the refresh
//! verdict (`osv::fix_path`), the parent-release walk and the held-dependency
//! rule ([`crate::held`]) share. Moved from the app's `osv/fix_path.rs`
//! unchanged (D-4), so the pure rule can judge a release's own ranges; the app
//! re-exports it under its old path.

use semver::{Version, VersionReq};

use crate::range::parse_version;

/// The spec without a pnpm peer suffix (`4.28.1(postcss@8.4.0)`), a leading
/// `=`/`v`, and surrounding whitespace.
pub fn strip_spec(spec: &str) -> &str {
    let spec = spec.split('(').next().unwrap_or(spec).trim();
    spec.trim_start_matches('=').trim().trim_start_matches('v')
}

/// Whether an npm requirement admits `version`. `None` when either side
/// cannot be read (dist-tags, `file:`/`git`/`workspace:` specs; an alias to
/// another name is read by its range). Never guesses: an unreadable
/// alternative in an `||` set makes a non-match unknown rather than false.
/// npm spells an exact pin as a bare version (`5.28.4`), unions with `||`,
/// ANDs comparators with spaces, allows a space after an operator
/// (`>= 4.21.0`) and writes hyphen ranges (`1.2.3 - 2.3.4`).
pub fn requirement_admits(spec: &str, version: &str) -> Option<bool> {
    let version = parse_version(version)?;
    let mut spec = spec.split('(').next().unwrap_or(spec).trim();
    if let Some(alias) = spec.strip_prefix("npm:") {
        spec = alias.rsplit_once('@').map(|(_, range)| range)?;
    }
    if spec.contains(':') || spec.contains('/') {
        return None;
    }
    let mut unknown = false;
    for alternative in spec.split("||") {
        match comparator_set(alternative.trim()) {
            Some(req) if req.matches(&version) => return Some(true),
            Some(_) => {}
            None => unknown = true,
        }
    }
    (!unknown).then_some(false)
}

/// One npm comparator set (space-separated, AND) as a semver `VersionReq`.
fn comparator_set(set: &str) -> Option<VersionReq> {
    if set.is_empty() || set == "*" || set.eq_ignore_ascii_case("x") {
        return Some(VersionReq::STAR);
    }
    if let Some((lo, hi)) = set.split_once(" - ") {
        let (lo, hi) = (strip_spec(lo), strip_spec(hi));
        Version::parse(lo).ok()?;
        Version::parse(hi).ok()?;
        return VersionReq::parse(&format!(">={lo}, <={hi}")).ok();
    }
    let mut comparators: Vec<String> = Vec::new();
    let mut pending_op = String::new();
    for token in set.split_whitespace() {
        if token
            .chars()
            .all(|c| matches!(c, '<' | '>' | '=' | '^' | '~'))
        {
            pending_op.push_str(token);
            continue;
        }
        let token = format!("{}{token}", std::mem::take(&mut pending_op));
        comparators.push(npm_comparator(&token)?);
    }
    if !pending_op.is_empty() || comparators.is_empty() {
        return None;
    }
    VersionReq::parse(&comparators.join(", ")).ok()
}

/// npm comparator -> Cargo comparator. A bare version is EXACT in npm (Cargo
/// would read it as a caret); a partial or x-range bare version is its X-range.
fn npm_comparator(token: &str) -> Option<String> {
    let op_len = token
        .find(|c: char| !matches!(c, '<' | '>' | '=' | '^' | '~'))
        .unwrap_or(token.len());
    let (op, rest) = token.split_at(op_len);
    let rest = rest.trim_start_matches('v');
    let parts: Vec<&str> = rest
        .split('.')
        .take_while(|p| !matches!(*p, "x" | "X" | "*"))
        .collect();
    if parts.is_empty() {
        return match op {
            "" | "=" | "^" | "~" | ">=" | "<=" => Some("*".to_string()),
            _ => None,
        };
    }
    let version = parts.join(".");
    Some(match op {
        "" => format!("={version}"),
        _ => format!("{op}{version}"),
    })
}
