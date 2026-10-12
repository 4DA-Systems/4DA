// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Opt-in fallback for the release card: the project's own GitHub release
//! notes, read only when the package archive yields nothing for the release
//! (Decision 1, option B, approved 2026-10-10; NETWORK.md §2m). A port of the
//! MCP server's `src/live/github-releases.ts` (6.2.0, `FOURDA_GITHUB_CHANGELOG`).
//!
//! Privacy rule: **off by default** (`settings.privacy.github_release_notes`).
//! Reading a project's releases tells GitHub which repositories the user
//! depends on, a party the default lane (option A, registry archives only)
//! never contacts. With the setting on:
//! - the repository comes from the package's own registry metadata (crates.io
//!   `repository`, npm `repository`), never guessed from the name, and only a
//!   `github.com` repository is read;
//! - the GitHub REST releases API is called unauthenticated (60 requests an
//!   hour); a rate limit is reported on the card and is never cached, and the
//!   client pauses until the limit resets instead of hammering it;
//! - a release counts only when its tag names an exact version of this
//!   package (`1.2.3`, `v1.2.3`, monorepo `pkg@1.2.3` / `pkg-v1.2.3`);
//! - the cache key carries [`GITHUB_PARSER_VERSION`], so a change to tag
//!   matching or body parsing re-reads instead of serving an old reading.
//!
//! Release bodies are third-party text: they go through the same parser and
//! sanitiser as an archive changelog (control and bidi characters stripped,
//! entries capped) and are shown as data. Nothing here acts on them.

use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};
use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::archive::MAX_FILE_BYTES;
use super::classify::re;
use super::fetch::{is_valid_package_name, is_valid_version, npm_manifest_url, Ecosystem};
use super::parse::{compare_versions, in_range, parse_changelog, ChangelogSection};
use super::{github_store, ReleaseChanges, ReleaseChangesStatus, ReleaseTarget};
use crate::db::Database;

/// Bump when tag matching or release-body parsing changes: it is part of the
/// cache key, so old readings are re-fetched rather than served.
pub(crate) const GITHUB_PARSER_VERSION: i64 = 1;

/// The only hosts this fallback contacts: the two registries (for the
/// repository URL in metadata they already serve) and GitHub's API.
pub(crate) const NOTES_HOSTS: &[&str] = &["api.github.com", "crates.io", "registry.npmjs.org"];

const USER_AGENT: &str = "4DA-Developer-OS/1.0 (https://4da.ai)";
const TIMEOUT: Duration = Duration::from_secs(15);
const PER_PAGE: usize = 100;
/// Pages of releases read per lookup (100 each). Unauthenticated, the API
/// allows 60 requests an hour, so this stays small.
const MAX_PAGES: usize = 3;
/// A response over this size is refused (a page of 100 release bodies).
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// How long to pause when a rate-limit answer names no reset time.
const DEFAULT_PAUSE_SECS: i64 = 15 * 60;

/// True when the user turned the fallback on (Settings → Privacy).
pub(crate) fn enabled() -> bool {
    crate::get_settings_manager()
        .lock()
        .get()
        .privacy
        .github_release_notes
}

/// One HTTP answer, with the headers the rate-limit check reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HttpReply {
    pub status: u16,
    pub ratelimit_remaining: Option<String>,
    pub ratelimit_reset: Option<i64>,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}

/// Why a request produced no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NetError {
    /// Network failure: retried later, never cached.
    Transient(String),
    /// The answer was over [`MAX_RESPONSE_BYTES`]: permanent for a release.
    TooLarge(String),
    /// The client is paused by an earlier rate limit.
    Paused(String),
}

/// The fallback's network layer. Injected so tests prove which requests are
/// (and are not) made.
#[async_trait]
pub(crate) trait NotesNet: Send + Sync {
    async fn get(&self, url: &str) -> Result<HttpReply, NetError>;
}

/// Why the fallback produced no notes this time. Neither is cached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotesError {
    RateLimited(String),
    Transient(String),
}

/// One release's notes, as cached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReleaseNote {
    pub version: String,
    pub date: Option<String>,
    pub body: String,
}

/// What a lookup found: the releases in (from, to], newest first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GithubNotes {
    /// `owner/repo`, when the registry links a GitHub repository.
    pub repo: Option<String>,
    pub notes: Vec<ReleaseNote>,
    /// The pages reached a release at or below `from`: the history is whole.
    pub reached_from: bool,
    /// Why there are no notes, in plain words.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure parts: repository URL, tag matching, release bodies
// ---------------------------------------------------------------------------

static SEGMENT: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z0-9_.-]{1,100}$"));

/// `owner/repo` of a repository URL on github.com, or `None`. Accepts the
/// forms registries carry (`https://`, `git+https://`, `git://`,
/// `git+ssh://git@`, `git@github.com:`, `github:o/r`, npm's bare `o/r`);
/// the host must be exactly github.com.
pub(crate) fn parse_github_repo(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim();
    let path = if let Some(short) = raw.strip_prefix("github:") {
        short.to_string()
    } else if let Some(rest) = raw.strip_prefix("git@github.com:") {
        rest.to_string()
    } else if raw.contains("://") {
        let normalized = raw.strip_prefix("git+").unwrap_or(raw);
        let url = reqwest::Url::parse(normalized).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        if host != "github.com" && host != "www.github.com" {
            return None;
        }
        url.path().trim_start_matches('/').to_string()
    } else if raw.matches('/').count() == 1 && !raw.contains(':') && !raw.contains('.') {
        // npm shorthand `owner/repo` means GitHub. A dot would be a host.
        raw.to_string()
    } else {
        return None;
    };
    let mut parts = path.trim_end_matches('/').split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.trim_end_matches(".git").to_string();
    let valid = |s: &str| SEGMENT.is_match(s) && s != "." && s != "..";
    (valid(&owner) && valid(&repo)).then_some((owner, repo))
}

static TAG: LazyLock<Regex> =
    LazyLock::new(|| re(r"^(.*?)(\d+\.\d+(?:\.\d+)*(?:[-.+]?[0-9A-Za-z][0-9A-Za-z.+-]*)?)$"));
static KNOWN_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)^(?:v|version[-_]?|release[-_]?|rel[-_]?)?$"));
static MONOREPO_SUFFIX: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)(?:[@/_-]v?|v)$"));

fn fold(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// How a tag names a version of the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TagMatch {
    /// `1.2.3`, `v1.2.3`, `release-1.2.3`: the repository's own line.
    Plain(String),
    /// `pkg@1.2.3`, `pkg-v1.2.3`, `@scope/pkg@1.2.3`: names this package.
    Named(String),
}

impl TagMatch {
    fn version(&self) -> &str {
        match self {
            Self::Plain(v) | Self::Named(v) => v,
        }
    }
}

/// The version a release tag names for `package`, or `None` when the tag is
/// another package's (monorepos) or not a version.
pub(crate) fn version_from_tag(tag: &str, package: &str) -> Option<TagMatch> {
    let caps = TAG.captures(tag.trim())?;
    let (prefix, version) = (caps.get(1)?.as_str(), caps.get(2)?.as_str().to_string());
    if KNOWN_PREFIX.is_match(prefix) {
        return Some(TagMatch::Plain(version));
    }
    let bare = MONOREPO_SUFFIX.replace(prefix, "");
    let name = package.rsplit('/').next().unwrap_or(package);
    let named = fold(&bare) == fold(package) || fold(&bare) == fold(name);
    (named && !bare.is_empty()).then_some(TagMatch::Named(version))
}

/// Release-body lines that are bookkeeping, not changes.
static NOISE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)made their first contribution|^\W*full changelog\W|^\W*(?:new )?contributors?\W*$")
});

/// One section per cached note, its entries parsed from the release body by
/// the archive changelog parser (a synthetic heading makes the body one
/// release; a body that repeats its own version heading keeps those entries).
pub(crate) fn notes_to_sections(notes: &[ReleaseNote]) -> Vec<ChangelogSection> {
    notes
        .iter()
        .map(|n| {
            let parsed = parse_changelog(&format!("# {}\n\n{}", n.version, n.body));
            let same = |v: &str| v.trim_start_matches('v') == n.version.trim_start_matches('v');
            let entries = parsed
                .into_iter()
                .enumerate()
                .filter(|(i, s)| *i == 0 || same(&s.version))
                .flat_map(|(_, s)| s.entries)
                .filter(|e| {
                    !NOISE.is_match(&e.text)
                        && !e
                            .under
                            .as_deref()
                            .is_some_and(|u| u.trim().eq_ignore_ascii_case("new contributors"))
                })
                .collect();
            ChangelogSection {
                version: n.version.clone(),
                date: n.date.clone(),
                entries,
            }
        })
        .collect()
}

/// The subset of the GitHub releases payload read here.
#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    published_at: Option<String>,
}

fn truncate_body(body: &str) -> String {
    if body.len() <= MAX_FILE_BYTES {
        return body.to_string();
    }
    let mut end = MAX_FILE_BYTES;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_string()
}

/// A 403/429 that is a rate limit, with when it lifts (epoch seconds).
pub(crate) fn rate_limit(reply: &HttpReply, now: i64) -> Option<(String, i64)> {
    if reply.status != 403 && reply.status != 429 {
        return None;
    }
    let exhausted = reply.ratelimit_remaining.as_deref() == Some("0");
    if !exhausted && reply.retry_after.is_none() && reply.status != 429 {
        return None;
    }
    let retry = reply
        .retry_after
        .as_deref()
        .and_then(|s| s.trim().parse::<i64>().ok());
    let until = match (reply.ratelimit_reset, retry) {
        (Some(reset), _) if reset > now => reset,
        (_, Some(secs)) if secs > 0 => now + secs,
        _ => now + DEFAULT_PAUSE_SECS,
    };
    let when = chrono::DateTime::from_timestamp(until, 0).map_or_else(
        || "later".to_string(),
        |d| d.format("%H:%M UTC").to_string(),
    );
    Some((
        format!(
            "GitHub's hourly limit for release notes is reached; 4DA will try again after {when}"
        ),
        until,
    ))
}

// ---------------------------------------------------------------------------
// Network: repository from registry metadata, then the releases API
// ---------------------------------------------------------------------------

fn json(reply: &HttpReply) -> Result<serde_json::Value, NotesError> {
    serde_json::from_slice(&reply.body)
        .map_err(|e| NotesError::Transient(format!("unreadable answer: {e}")))
}

fn net_err(e: NetError) -> Result<HttpReply, NotesError> {
    match e {
        NetError::Transient(m) | NetError::TooLarge(m) => Err(NotesError::Transient(m)),
        NetError::Paused(m) => Err(NotesError::RateLimited(m)),
    }
}

/// The repository URL the package's registry metadata names, if any.
async fn registry_repository(
    net: &dyn NotesNet,
    t: &ReleaseTarget,
) -> Result<Option<String>, NotesError> {
    let url = match t.eco {
        Ecosystem::Crates => format!("https://crates.io/api/v1/crates/{}", t.package),
        Ecosystem::Npm => npm_manifest_url(&t.package, &t.to),
    };
    let reply = match net.get(&url).await {
        Ok(r) => r,
        Err(e) => net_err(e)?,
    };
    if reply.status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&reply.status) {
        return Err(NotesError::Transient(format!(
            "registry answered HTTP {}",
            reply.status
        )));
    }
    let doc = json(&reply)?;
    let field = match t.eco {
        Ecosystem::Crates => doc.get("crate").and_then(|c| c.get("repository")),
        Ecosystem::Npm => doc.get("repository"),
    };
    Ok(field.and_then(|f| {
        f.as_str()
            .or_else(|| f.get("url").and_then(|u| u.as_str()))
            .map(str::to_string)
    }))
}

fn no_notes(repo: Option<String>, reason: String) -> GithubNotes {
    GithubNotes {
        repo,
        reason: Some(reason),
        ..GithubNotes::default()
    }
}

/// Read the GitHub release notes for `t`, uncached. Paging stops at a
/// release at or below `t.from`, or after [`MAX_PAGES`].
pub(crate) async fn fetch_notes(
    net: &dyn NotesNet,
    t: &ReleaseTarget,
    now: i64,
) -> Result<GithubNotes, NotesError> {
    if !is_valid_package_name(&t.package, t.eco) || !is_valid_version(&t.to) {
        return Ok(no_notes(
            None,
            format!("{} {} is not a valid package/version", t.package, t.to),
        ));
    }
    let Some(raw) = registry_repository(net, t).await? else {
        return Ok(no_notes(
            None,
            "the registry entry links no repository".into(),
        ));
    };
    let Some((owner, repo)) = parse_github_repo(&raw) else {
        return Ok(no_notes(
            None,
            "the registry entry links a repository that is not on github.com".into(),
        ));
    };
    let where_ = format!("{owner}/{repo}");
    // Plain (`v1.2.3`) and named (`pkg@1.2.3`) tags are kept apart: in a
    // monorepo that tags this package by name, the bare tags are another
    // package's, so neither its notes nor its old versions count here.
    let (mut plain, mut named) = (Vec::new(), Vec::new());
    let (mut saw_named, mut reached_plain, mut reached_named) = (false, false, false);
    for page in 1..=MAX_PAGES {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/releases?per_page={PER_PAGE}&page={page}"
        );
        let reply = match net.get(&url).await {
            Ok(r) => r,
            Err(NetError::TooLarge(m)) => return Ok(no_notes(Some(where_), m)),
            Err(e) => net_err(e)?,
        };
        if let Some((message, _)) = rate_limit(&reply, now) {
            return Err(NotesError::RateLimited(message));
        }
        if reply.status == 404 {
            return Ok(no_notes(
                Some(where_.clone()),
                format!("github.com/{where_} has no public releases"),
            ));
        }
        if !(200..300).contains(&reply.status) {
            return Err(NotesError::Transient(format!(
                "GitHub answered HTTP {}",
                reply.status
            )));
        }
        let batch: Vec<ApiRelease> = serde_json::from_slice(&reply.body)
            .map_err(|e| NotesError::Transient(format!("unreadable releases payload: {e}")))?;
        let full_page = batch.len() >= PER_PAGE;
        for r in batch {
            let (Some(tag), false) = (r.tag_name.as_deref(), r.draft) else {
                continue;
            };
            let Some(m) = version_from_tag(tag, &t.package) else {
                continue;
            };
            let at_or_below_from = matches!(
                compare_versions(m.version(), &t.from),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            );
            match m {
                TagMatch::Named(_) => {
                    saw_named = true;
                    reached_named |= at_or_below_from;
                }
                TagMatch::Plain(_) => reached_plain |= at_or_below_from,
            }
            if !in_range(m.version(), &t.from, &t.to) {
                continue;
            }
            let note = ReleaseNote {
                version: m.version().to_string(),
                date: r
                    .published_at
                    .as_deref()
                    .map(|d| d.chars().take(10).collect()),
                body: truncate_body(r.body.as_deref().unwrap_or("")),
            };
            match m {
                TagMatch::Plain(_) => plain.push(note),
                TagMatch::Named(_) => named.push(note),
            }
        }
        if !full_page || reached_named || (!saw_named && reached_plain) {
            break;
        }
    }
    let (notes, reached_from) = if saw_named {
        (named, reached_named)
    } else {
        (plain, reached_plain)
    };
    let reason = notes.is_empty().then(|| {
        format!(
            "github.com/{where_} has no release for {} after {} up to {}",
            t.package, t.from, t.to
        )
    });
    Ok(GithubNotes {
        repo: Some(where_),
        notes,
        reached_from,
        reason,
    })
}

// ---------------------------------------------------------------------------
// The fallback
// ---------------------------------------------------------------------------

/// True when the archive card says the package told us nothing about this
/// release. `Unavailable` (registry unreachable) is retried, not replaced.
pub(crate) fn archive_yields_nothing(status: ReleaseChangesStatus) -> bool {
    matches!(
        status,
        ReleaseChangesStatus::NoChangelog
            | ReleaseChangesStatus::Unparsed
            | ReleaseChangesStatus::Refused
            | ReleaseChangesStatus::NoSectionsInRange
    )
}

/// The notes for `t`, from the cache or GitHub. Rate limits and network
/// failures are returned and never cached.
pub(crate) async fn notes_for(
    db: &Database,
    t: &ReleaseTarget,
    net: &dyn NotesNet,
) -> Result<GithubNotes, NotesError> {
    if let Ok(Some(hit)) = github_store::get(db, t) {
        return Ok(hit);
    }
    let notes = fetch_notes(net, t, chrono::Utc::now().timestamp()).await?;
    if let Err(e) = github_store::put(db, t, &notes) {
        warn!(target: "4da::release_changelog", package = %t.package, "github notes cache write failed: {e}");
    }
    Ok(notes)
}

/// The card built from GitHub notes, or `None` when none fall in range.
pub(crate) fn summarize_github(t: &ReleaseTarget, notes: &GithubNotes) -> Option<ReleaseChanges> {
    let repo = notes.repo.as_deref()?;
    let sections = notes_to_sections(&notes.notes);
    let selected: Vec<&ChangelogSection> = sections
        .iter()
        .filter(|s| in_range(&s.version, &t.from, &t.to))
        .collect();
    if selected.is_empty() {
        return None;
    }
    let has_target = selected
        .iter()
        .any(|s| compare_versions(&s.version, &t.to) == Some(std::cmp::Ordering::Equal));
    let mut out = super::base_changes(t, ReleaseChangesStatus::Found);
    out.file = Some("GitHub releases".into());
    out.source_url = Some(format!("https://github.com/{repo}/releases"));
    out.from_github = true;
    out.covers_range = notes.reached_from && has_target;
    super::tally(&mut out, &selected);
    Some(out)
}

fn with_reason(mut card: ReleaseChanges, extra: &str) -> ReleaseChanges {
    card.reason = Some(match card.reason.take() {
        Some(r) => format!("{r}; {extra}"),
        None => extra.to_string(),
    });
    card
}

/// The release card after the opt-in fallback: `archive_card` unchanged
/// unless `enabled` AND the archive yields nothing for this release. With
/// the setting off, `net` is never called.
pub(crate) async fn with_github_fallback(
    db: &Database,
    t: &ReleaseTarget,
    archive_card: ReleaseChanges,
    enabled: bool,
    net: &dyn NotesNet,
) -> ReleaseChanges {
    if !enabled || !archive_yields_nothing(archive_card.status) {
        return archive_card;
    }
    match notes_for(db, t, net).await {
        Ok(notes) => match summarize_github(t, &notes) {
            Some(card) => card,
            None => {
                let why = notes
                    .reason
                    .unwrap_or_else(|| "no GitHub release notes".into());
                with_reason(archive_card, &why)
            }
        },
        Err(NotesError::RateLimited(m) | NotesError::Transient(m)) => {
            debug!(target: "4da::release_changelog", package = %t.package, "github notes deferred: {m}");
            with_reason(
                archive_card,
                &format!("GitHub release notes could not be read: {m}"),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// The live network layer
// ---------------------------------------------------------------------------

/// True when `url` is https on one of [`NOTES_HOSTS`].
pub(crate) fn is_notes_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.port().is_none()
            && u.host_str().is_some_and(|h| NOTES_HOSTS.contains(&h))
    })
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 3 && is_notes_url(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .unwrap_or_else(|e| {
            warn!(target: "4da::release_changelog", "notes client build failed: {e}; using default");
            reqwest::Client::new()
        })
});

/// The real network, unauthenticated. After a GitHub rate limit it sends
/// nothing to GitHub until the limit resets.
pub(crate) struct LiveNet {
    paused_until: AtomicI64,
}

pub(crate) static LIVE_NET: LiveNet = LiveNet {
    paused_until: AtomicI64::new(0),
};

fn header(h: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    h.get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

#[async_trait]
impl NotesNet for LiveNet {
    async fn get(&self, url: &str) -> Result<HttpReply, NetError> {
        if !is_notes_url(url) {
            return Err(NetError::Transient(format!("{url} is not an allowed host")));
        }
        let github = url.starts_with("https://api.github.com/");
        let now = chrono::Utc::now().timestamp();
        if github && self.paused_until.load(AtomicOrdering::SeqCst) > now {
            return Err(NetError::Paused(
                "GitHub's hourly limit for release notes is reached; 4DA will try again later"
                    .into(),
            ));
        }
        let mut req = CLIENT.get(url).header("User-Agent", USER_AGENT);
        if github {
            req = req
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28");
        }
        let mut response = req
            .send()
            .await
            .map_err(|e| NetError::Transient(format!("request failed: {e}")))?;
        let h = response.headers();
        let mut reply = HttpReply {
            status: response.status().as_u16(),
            ratelimit_remaining: header(h, "x-ratelimit-remaining"),
            ratelimit_reset: header(h, "x-ratelimit-reset").and_then(|s| s.parse().ok()),
            retry_after: header(h, "retry-after"),
            body: Vec::new(),
        };
        if github {
            if let Some((_, until)) = rate_limit(&reply, now) {
                self.paused_until.store(until, AtomicOrdering::SeqCst);
                return Ok(reply);
            }
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
        {
            return Err(NetError::TooLarge(format!(
                "the answer from {url} is over the size cap"
            )));
        }
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| NetError::Transient(format!("download interrupted: {e}")))?
        {
            if reply.body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(NetError::TooLarge(format!(
                    "the answer from {url} is over the size cap"
                )));
            }
            reply.body.extend_from_slice(&chunk);
        }
        Ok(reply)
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
