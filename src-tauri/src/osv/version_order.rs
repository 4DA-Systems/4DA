// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Version ordering for OSV affected ranges — one primitive for the matcher
//! and the fix-target walk.
//!
//! Measured 2026-10-10 against osv-scanner / govulncheck / pip-audit on 19
//! public repositories, three orderings were wrong:
//! - **`introduced: "0"` was the version 0.0.0.** OSV defines `"0"` as "since
//!   the beginning", below every version. A Go pseudo-version
//!   (`v0.0.0-20190308221718-c2843e01d9a2`) is a semver PRE-RELEASE of 0.0.0,
//!   so it sorted below the bound and every advisory open since the first
//!   commit read "not affected".
//! - **Build metadata decided order.** `semver::Version`'s `Ord` compares
//!   build metadata as a tiebreaker; OSV (and Go's `+incompatible`) treat it
//!   as no part of the precedence.
//! - **PyPI is PEP 440, not semver.** `1.0.post1`, `2.0rc1`, `1.2.3.4` and
//!   `1!2.0` failed the semver parse, so the copy matched "conservatively" and
//!   was never confirmed.
//!
//! The rule for a pair of versions: a PyPI version is read ONLY as PEP 440
//! ([`OrderedVersion::parse_in`]), so a pair with a PyPI side orders by
//! PEP 440. Otherwise compare as semver when BOTH parse as semver (npm,
//! crates.io, Go), else as PEP 440 when both parse as PEP 440, else the pair
//! is undecidable and the caller stays conservative.
//!
//! Rule vectors 2026-10-12 (`tests/conformance/vectors`, vo-013..015 and
//! vo-042..044): reading a PyPI pair as semver whenever both sides parsed as
//! semver put `1.0.0-1` and `1.0.0-post1` (post releases) BELOW `1.0.0`, and
//! `1.0.0-a10` below `1.0.0-a9`; and the PEP 440 local label was dropped, so
//! `1.0+abc.5` equalled `1.0`.

use std::cmp::Ordering;
use std::sync::OnceLock;

use regex::Regex;
use semver::{BuildMetadata, Version};

/// A PEP 440 version, local label included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pep440 {
    epoch: u64,
    release: Vec<u64>,
    /// `(rank, n)`: a/alpha = 0, b/beta = 1, c/rc/pre/preview = 2.
    pre: Option<(u8, u64)>,
    post: Option<u64>,
    dev: Option<u64>,
    /// The local label (`+cpu`, `+abc.5`), empty when there is none. A
    /// version with one sorts above the same version without (PEP 440).
    local: Vec<LocalSegment>,
}

/// One segment of a local label. PEP 440: a numeric segment sorts above an
/// alphanumeric one, numbers compare numerically and strings
/// lexicographically (case-insensitive), and when one label is a prefix of
/// the other the longer is greater. The variant order gives exactly that.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum LocalSegment {
    Text(String),
    Number(u64),
}

fn local_segments(label: &str) -> Vec<LocalSegment> {
    label
        .split(['.', '-', '_'])
        .map(|seg| match seg.parse::<u64>() {
            Ok(n) if seg.bytes().all(|b| b.is_ascii_digit()) => LocalSegment::Number(n),
            _ => LocalSegment::Text(seg.to_ascii_lowercase()),
        })
        .collect()
}

fn pep440_regex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^v?(?:(\d+)!)?(\d+(?:\.\d+)*)(?:[-_.]?(a|alpha|b|beta|c|rc|pre|preview)[-_.]?(\d*))?(?:(?:[-_.]?(?:post|rev|r)[-_.]?(\d*))|-(\d+))?(?:[-_.]?dev[-_.]?(\d*))?(?:\+([a-z0-9]+(?:[-_.][a-z0-9]+)*))?$",
        )
        .ok()
    })
    .as_ref()
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() {
        Some(0)
    } else {
        text.parse().ok()
    }
}

/// Parse a PEP 440 version, or `None` when it is not one.
pub(crate) fn parse_pep440(raw: &str) -> Option<Pep440> {
    let caps = pep440_regex()?.captures(raw.trim())?;
    let group = |i: usize| caps.get(i).map(|m| m.as_str());
    let release = group(2)?
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    let pre = match group(3) {
        Some(label) => {
            let rank = match label.to_ascii_lowercase().as_str() {
                "a" | "alpha" => 0,
                "b" | "beta" => 1,
                _ => 2,
            };
            Some((rank, number(group(4).unwrap_or(""))?))
        }
        None => None,
    };
    let post = match (group(5), group(6)) {
        (Some(n), _) => Some(number(n)?),
        (None, Some(n)) => Some(number(n)?),
        (None, None) => None,
    };
    let dev = match group(7) {
        Some(n) => Some(number(n)?),
        None => None,
    };
    Some(Pep440 {
        epoch: group(1).map_or(Some(0), number)?,
        release,
        pre,
        post,
        dev,
        local: group(8).map(local_segments).unwrap_or_default(),
    })
}

/// PEP 440 sort order within one release:
/// `X.devN < X.aN.devM < X.aN < X.aN.postM < X.bN < X.rcN < X < X+local < X.postN.devM < X.postN`,
/// the local label deciding last.
fn cmp_pep440(a: &Pep440, b: &Pep440) -> Ordering {
    let release_len = a.release.len().max(b.release.len());
    let release = |v: &Pep440, i: usize| v.release.get(i).copied().unwrap_or(0);
    a.epoch
        .cmp(&b.epoch)
        .then_with(|| {
            (0..release_len)
                .map(|i| release(a, i).cmp(&release(b, i)))
                .find(|o| o.is_ne())
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| pep440_key(a).cmp(&pep440_key(b)))
        .then_with(|| a.local.cmp(&b.local))
}

/// The tail of PEP 440's sort key: pre-release phase (a dev-only release
/// sorts below every pre-release, a final above), then post (none lowest),
/// then dev (none highest).
fn pep440_key(v: &Pep440) -> (i8, u64, i128, i128) {
    let phase = match v.pre {
        Some((rank, _)) => rank as i8,
        None if v.post.is_none() && v.dev.is_some() => -1,
        None => 3,
    };
    let pre_n = v.pre.map_or(0, |(_, n)| n);
    let post = v.post.map_or(-1, i128::from);
    let dev = v.dev.map_or(i128::MAX, i128::from);
    (phase, pre_n, post, dev)
}

/// One version, readable under semver, PEP 440, or both.
#[derive(Debug, Clone)]
pub(crate) struct OrderedVersion {
    semver: Option<Version>,
    pep440: Option<Pep440>,
}

impl OrderedVersion {
    /// `None` when the text is a version under neither scheme.
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let semver = super::matching::parse_version(raw).map(|mut v| {
            v.build = BuildMetadata::EMPTY;
            v
        });
        let pep440 = parse_pep440(raw);
        (semver.is_some() || pep440.is_some()).then_some(Self { semver, pep440 })
    }

    /// [`Self::parse`] for a version of `ecosystem`. A PyPI version that
    /// reads as PEP 440 is ONLY PEP 440: `1.0.0-1` is the post release
    /// `1.0.0.post1`, `1.0.0-a10` is above `1.0.0-a9`, and `2.3.0+cpu` is
    /// above `2.3.0`, where semver reads all three differently. Any other
    /// ecosystem (or an unknown one) parses as [`Self::parse`].
    pub(crate) fn parse_in(raw: &str, ecosystem: &str) -> Option<Self> {
        if is_pypi(ecosystem) {
            if let Some(pep440) = parse_pep440(raw) {
                return Some(Self {
                    semver: None,
                    pep440: Some(pep440),
                });
            }
        }
        Self::parse(raw)
    }

    /// A pre-release (or a PEP 440 dev release).
    pub(crate) fn is_prerelease(&self) -> bool {
        match (&self.semver, &self.pep440) {
            (Some(v), _) => !v.pre.is_empty(),
            (None, Some(p)) => p.pre.is_some() || p.dev.is_some(),
            (None, None) => false,
        }
    }

    /// The pair's order, or `None` when no scheme reads both sides.
    pub(crate) fn compare(&self, other: &Self) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (&self.semver, &other.semver) {
            return Some(a.cmp(b));
        }
        if let (Some(a), Some(b)) = (&self.pep440, &other.pep440) {
            return Some(cmp_pep440(a, b));
        }
        None
    }
}

/// Whether `ecosystem` names PyPI, under any alias the app uses.
pub(crate) fn is_pypi(ecosystem: &str) -> bool {
    crate::ecosystem::Ecosystem::parse(ecosystem).map(crate::ecosystem::Ecosystem::osv_name)
        == Some("PyPI")
}

/// The lower edge of an affected window.
#[derive(Debug, Clone)]
pub(crate) enum Lower {
    /// OSV's `introduced: "0"`: every version, pre-releases of 0.0.0 included.
    Unbounded,
    At(OrderedVersion),
}

impl Lower {
    /// The bound as `ecosystem` reads it; `None` when it is unreadable.
    pub(crate) fn parse_in(raw: &str, ecosystem: &str) -> Option<Self> {
        if raw.trim() == "0" {
            return Some(Lower::Unbounded);
        }
        OrderedVersion::parse_in(raw, ecosystem).map(Lower::At)
    }
}

/// The upper edge of an affected window: `fixed` (exclusive) or
/// `last_affected` (inclusive).
pub(crate) struct Upper<'a> {
    pub version: &'a OrderedVersion,
    pub inclusive: bool,
}

/// Whether `user` lies in `[lower, upper)` (or `[lower, upper]`). `None` when
/// an edge cannot be compared with `user`.
pub(crate) fn in_window(
    user: &OrderedVersion,
    lower: &Lower,
    upper: Option<Upper<'_>>,
) -> Option<bool> {
    let above_lower = match lower {
        Lower::Unbounded => true,
        Lower::At(bound) => user.compare(bound)?.is_ge(),
    };
    let below_upper = match upper {
        None => true,
        Some(Upper { version, inclusive }) => {
            let order = user.compare(version)?;
            if inclusive {
                order.is_le()
            } else {
                order.is_lt()
            }
        }
    };
    Some(above_lower && below_upper)
}

#[cfg(test)]
#[path = "version_order_tests.rs"]
mod tests;
