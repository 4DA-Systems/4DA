// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Whether a version constraint admits a version, in the ecosystem's own
//! syntax: the membership half of [`crate::range_check`].
//!
//! The constraint is what an agent was told to write: `"7.x"`, `"^1.20"`,
//! `"~=3.1"`, `"3.1.*"`, `">=2,<3"`, `"v1"`. Each ecosystem reads it the way
//! its own tool does, plus the shorthand every agent writes:
//! - **npm**: an npm range ([`crate::npm_range::requirement_admits`]: a bare
//!   version is an exact pin, `||` unions, spaces AND, x-ranges, hyphens).
//! - **crates.io**: a Cargo requirement first (`0.17` means `^0.17`, commas
//!   AND), then the npm spellings agents also write (`0.17.x`).
//! - **PyPI**: PEP 440 specifiers (`==`, `!=` with an optional `.*` prefix
//!   match, `>=`, `<=`, `>`, `<`, `~=`, `===`), comma-separated; `3.1.*`,
//!   `3.1.x` or `3.x` with no operator is `==3.1.*`.
//! - **Go**: Go has no range syntax of its own, so a semver range is read
//!   over `v`-less versions; `v1` is the v1 major, `v0` the v0 line, and a
//!   comma ANDs like a space.
//!
//! The same rules as `@4da/mcp-server` 6.2.x (`dependency-check-versions.ts`
//! `rangeAdmits` / `pep440SpecAdmits`). `None` means the constraint or the
//! version cannot be read: never a guess.

use std::cmp::Ordering;

use crate::ecosystem::osv_name;
use crate::fix_path::Requirement;
use crate::npm_range::requirement_admits;
use crate::version_order::{cmp_pep440, parse_pep440, Pep440};

/// Whether `constraint` admits `version` in `ecosystem` (any alias the app
/// stores). `None` when either side is unreadable, or the ecosystem is one
/// this rule does not read constraints for.
pub fn constraint_admits(ecosystem: &str, constraint: &str, version: &str) -> Option<bool> {
    match osv_name(ecosystem)? {
        "npm" => requirement_admits(constraint, version),
        "crates.io" => Requirement::cargo(constraint)
            .admits(version)
            .or_else(|| requirement_admits(constraint, version)),
        "PyPI" => pep440_spec_admits(&pypi_shorthand(constraint.trim()), version),
        "Go" => {
            let range = go_range(constraint);
            let version = version.trim();
            let version = version.strip_prefix('v').unwrap_or(version);
            let version = version.strip_suffix("+incompatible").unwrap_or(version);
            requirement_admits(&range, version)
        }
        _ => None,
    }
}

/// `3.1.x` / `3.x` / `3.1.*` with no operator -> `==3.1.*`; anything else as
/// written.
fn pypi_shorthand(text: &str) -> String {
    let body = text.strip_prefix('v').unwrap_or(text);
    let wildcard = body
        .strip_suffix(".x")
        .or_else(|| body.strip_suffix(".X"))
        .or_else(|| body.strip_suffix(".*"));
    match wildcard {
        Some(prefix)
            if !prefix.is_empty()
                && prefix
                    .split('.')
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) =>
        {
            format!("=={prefix}.*")
        }
        _ => text.to_string(),
    }
}

/// A Go constraint as an npm range over `v`-less versions: a `v` before a
/// digit is dropped where a version starts (the text's start, or after
/// whitespace, `,`, an operator, `|` or `-`), and `,` ANDs like a space.
fn go_range(constraint: &str) -> String {
    let chars: Vec<char> = constraint.chars().collect();
    let mut out = String::with_capacity(constraint.len());
    for (i, &c) in chars.iter().enumerate() {
        let starts_version = i == 0
            || chars.get(i - 1).is_some_and(|p| {
                p.is_whitespace() || matches!(p, ',' | '<' | '>' | '=' | '~' | '^' | '|' | '-')
            });
        let before_digit = chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if c == 'v' && starts_version && before_digit {
            continue;
        }
        out.push(c);
    }
    // `,\s*` -> one space.
    let mut parts = out.split(',');
    let first = parts.next().unwrap_or_default().to_string();
    parts.fold(first, |acc, part| format!("{acc} {}", part.trim_start()))
}

/// PEP 440 version specifiers: comma-separated clauses of `==`, `!=` (both
/// with an optional `.*` prefix match), `>=`, `<=`, `>`, `<`, `~=` and
/// `===`, all of which must hold. `None` when a clause or the version is
/// unreadable. Local labels take no part in a comparison.
pub fn pep440_spec_admits(spec: &str, version: &str) -> Option<bool> {
    let v = without_local(parse_pep440(version)?);
    let clauses: Vec<&str> = spec
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect();
    if clauses.is_empty() {
        return None;
    }
    let mut admitted = true;
    for clause in clauses {
        admitted &= clause_admits(clause, &v, version)?;
    }
    Some(admitted)
}

/// One specifier clause. Every clause is read even after one fails, so an
/// unreadable clause anywhere makes the whole specifier unreadable.
fn clause_admits(clause: &str, v: &Pep440, raw_version: &str) -> Option<bool> {
    let op_len = clause
        .find(|c: char| !matches!(c, '=' | '!' | '~' | '<' | '>'))
        .unwrap_or(clause.len());
    let (op, target) = (&clause[..op_len], clause[op_len..].trim());
    if target.is_empty() || target.contains(char::is_whitespace) {
        return None;
    }
    if op == "===" {
        return Some(target == raw_version);
    }
    let (target, prefix) = match target.strip_suffix(".*") {
        Some(t) => (t, true),
        None => (target, false),
    };
    let t = without_local(parse_pep440(target)?);
    if prefix {
        return match op {
            "==" => Some(matches_prefix(v, &t)),
            "!=" => Some(!matches_prefix(v, &t)),
            _ => None,
        };
    }
    let order = cmp_pep440(v, &t);
    Some(match op {
        "~=" => {
            if t.release.len() < 2 {
                return None;
            }
            let series = Pep440 {
                release: t.release[..t.release.len() - 1].to_vec(),
                pre: None,
                post: None,
                dev: None,
                ..t.clone()
            };
            order.is_ge() && matches_prefix(v, &series)
        }
        "==" => order == Ordering::Equal,
        "!=" => order != Ordering::Equal,
        ">=" => order.is_ge(),
        "<=" => order.is_le(),
        ">" => order.is_gt(),
        "<" => order.is_lt(),
        _ => return None,
    })
}

fn without_local(mut v: Pep440) -> Pep440 {
    v.local.clear();
    v
}

/// `==X.Y.*`: the same epoch, and the release starts with X.Y (missing parts
/// read as 0).
fn matches_prefix(v: &Pep440, prefix: &Pep440) -> bool {
    v.epoch == prefix.epoch
        && prefix
            .release
            .iter()
            .enumerate()
            .all(|(i, n)| v.release.get(i).copied().unwrap_or(0) == *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_and_go_ranges() {
        assert_eq!(constraint_admits("npm", "7.x", "7.5.13"), Some(true));
        assert_eq!(constraint_admits("npm", "7.x", "8.0.0"), Some(false));
        assert_eq!(
            constraint_admits("npm", ">=7.5.11 <8", "7.5.12"),
            Some(true)
        );
        assert_eq!(constraint_admits("npm", "8.17.x", "8.18.0"), Some(false));
        assert_eq!(constraint_admits("npm", "latest", "1.0.0"), None);
        assert_eq!(constraint_admits("Go", "v0", "v0.61.0"), Some(true));
        assert_eq!(constraint_admits("Go", "v1", "v0.61.0"), Some(false));
        assert_eq!(
            constraint_admits("Go", ">=v0.55.0, <v0.60.0", "v0.59.0"),
            Some(true)
        );
        assert_eq!(constraint_admits("Go", "~0.60.0", "v0.61.0"), Some(false));
        assert_eq!(
            constraint_admits("Go", "v2", "v2.1.0+incompatible"),
            Some(true)
        );
        assert_eq!(go_range(">=v1.2.0,  <v2"), ">=1.2.0 <2");
    }

    #[test]
    fn cargo_reads_its_own_syntax_then_npm_spellings() {
        // A bare Cargo requirement is a caret.
        assert_eq!(constraint_admits("crates.io", "0.17", "0.17.9"), Some(true));
        assert_eq!(
            constraint_admits("crates.io", "0.17", "0.18.0"),
            Some(false)
        );
        assert_eq!(
            constraint_admits("crates.io", ">=1.2, <2", "1.9.0"),
            Some(true)
        );
        assert_eq!(
            constraint_admits("crates.io", "0.17.x", "0.17.2"),
            Some(true)
        );
    }

    #[test]
    fn pep440_specifiers() {
        let admits = |spec: &str, v: &str| constraint_admits("PyPI", spec, v);
        assert_eq!(admits("==3.1.*", "3.1.6"), Some(true));
        assert_eq!(admits("==3.1.*", "3.10.0"), Some(false));
        assert_eq!(admits("3.1.*", "3.1.2"), Some(true), "shorthand");
        assert_eq!(admits("3.x", "3.0.3"), Some(true), "shorthand");
        assert_eq!(admits("~=3.1", "3.9"), Some(true));
        assert_eq!(admits("~=3.1", "4.0"), Some(false));
        assert_eq!(admits("~=3.1.2", "3.1.4"), Some(true));
        assert_eq!(admits("~=3.1.2", "3.2.0"), Some(false));
        assert_eq!(admits("~=3", "3.1"), None, "~= needs two release parts");
        assert_eq!(admits("!=3.1.6,>=3.0", "3.1.6"), Some(false));
        assert_eq!(admits("!=3.1.*", "3.0.3"), Some(true));
        assert_eq!(admits(">=3.0,<3.1.6", "3.1.5"), Some(true));
        assert_eq!(
            admits("==3.1.6", "3.1.6+local"),
            Some(true),
            "local labels do not compare"
        );
        assert_eq!(admits("===3.1.6", "3.1.6"), Some(true));
        assert_eq!(admits(">3.1", "3.1.post1"), Some(true));
        assert_eq!(
            admits(">=3.0,nope", "3.1"),
            None,
            "an unreadable clause is unreadable"
        );
        assert_eq!(admits(">=3.0", "not a version"), None);
        assert_eq!(admits(">=3.0", "1!1.0"), Some(true), "epoch");
    }

    #[test]
    fn other_ecosystems_are_not_read() {
        assert_eq!(constraint_admits("Maven", "[1.0,2.0)", "1.5"), None);
    }
}
