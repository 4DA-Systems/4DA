// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The lane's network path: the registry archive for ONE published version.
//!
//! Privacy rule (Decision 1, option A — NETWORK.md §1e): the only hosts are the
//! package's own registry — `static.crates.io` (crates.io's download CDN) for a
//! crate, `registry.npmjs.org` for an npm package — which already receive the
//! user's package names from the release watch. What is sent is the package
//! name and the PUBLIC version being announced; the user's installed version
//! never leaves the machine (the range is cut locally). Never GitHub, never a
//! docs site, even when the archive ships no changelog.
//!
//! Every URL is built here and checked against [`ARCHIVE_HOSTS`] before a
//! request goes out, so a registry document pointing a tarball elsewhere is
//! refused rather than followed.

use std::sync::LazyLock;
use std::time::Duration;

use super::archive::{ArchiveError, MAX_COMPRESSED_BYTES};

/// The only hosts an archive is downloaded from.
pub(crate) const ARCHIVE_HOSTS: &[&str] = &["static.crates.io", "registry.npmjs.org"];

/// crates.io asks for a descriptive User-Agent; the same one the release
/// watch already sends (`sources/crates_io.rs`).
const CRATES_USER_AGENT: &str = "4DA-Developer-OS/1.0 (https://4da.ai)";
const NPM_USER_AGENT: &str = "4DA-Developer-OS/1.0";
const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(30);
const MANIFEST_TIMEOUT: Duration = Duration::from_secs(15);

/// The lane's own client: it follows a redirect only to another archive
/// host, so a registry answer pointing elsewhere is never contacted (the
/// shared client follows any public redirect).
static ARCHIVE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(ARCHIVE_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 3 && is_archive_url(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!(target: "4da::release_changelog", "archive client build failed: {e}; using default");
            reqwest::Client::new()
        })
});

/// The registries the lane reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ecosystem {
    Crates,
    Npm,
}

impl Ecosystem {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Crates => "crates.io",
            Self::Npm => "npm",
        }
    }

    /// The lane's ecosystem for a `source_items.source_type`.
    pub(crate) fn from_source_type(source_type: &str) -> Option<Self> {
        match source_type {
            "crates_io" | "crates" => Some(Self::Crates),
            "npm_registry" | "npm" => Some(Self::Npm),
            _ => None,
        }
    }

    fn user_agent(self) -> &'static str {
        match self {
            Self::Crates => CRATES_USER_AGENT,
            Self::Npm => NPM_USER_AGENT,
        }
    }
}

/// Why a fetch did not produce archive bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchError {
    /// The archive itself is refused (cap, format, off-registry URL, 404):
    /// permanent for a published version, so it is cached.
    Refused(String),
    /// The network or the registry failed this time: retried next cycle,
    /// never cached.
    Transient(String),
}

impl From<ArchiveError> for FetchError {
    fn from(e: ArchiveError) -> Self {
        Self::Refused(e.0)
    }
}

/// Package names the registries accept; anything else is never put in a URL.
pub(crate) fn is_valid_package_name(name: &str, eco: Ecosystem) -> bool {
    let ok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~');
    match eco {
        Ecosystem::Crates => {
            name.len() <= 64
                && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }
        Ecosystem::Npm => {
            let bare = match name.strip_prefix('@') {
                Some(scoped) => match scoped.split_once('/') {
                    Some((scope, pkg)) if !scope.is_empty() && scope.chars().all(ok_char) => pkg,
                    _ => return false,
                },
                None => name,
            };
            name.len() <= 214
                && !bare.is_empty()
                && !bare.starts_with('.')
                && bare.chars().all(ok_char)
        }
    }
}

/// A version safe to put in a URL path: semver characters only.
pub(crate) fn is_valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version.chars().next().is_some_and(|c| c.is_ascii_digit())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

/// True when `url` is https on one of [`ARCHIVE_HOSTS`].
pub(crate) fn is_archive_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.port().is_none()
            && u.host_str().is_some_and(|h| ARCHIVE_HOSTS.contains(&h))
    })
}

/// The crates.io archive URL for one version.
pub(crate) fn crate_archive_url(name: &str, version: &str) -> String {
    format!("https://static.crates.io/crates/{name}/{name}-{version}.crate")
}

/// The npm version-manifest URL (`/{name}/{version}`, the same document shape
/// the release watch reads at `/{name}/latest`).
pub(crate) fn npm_manifest_url(name: &str, version: &str) -> String {
    format!(
        "https://registry.npmjs.org/{}/{version}",
        name.replace('/', "%2F")
    )
}

/// `dist.tarball` from an npm version manifest, if it is on the registry.
pub(crate) fn npm_tarball_from_manifest(manifest: &serde_json::Value) -> Option<String> {
    let url = manifest.get("dist")?.get("tarball")?.as_str()?;
    is_archive_url(url).then(|| url.to_string())
}

async fn get(
    url: &str,
    eco: Ecosystem,
    timeout: Duration,
) -> Result<reqwest::Response, FetchError> {
    if !is_archive_url(url) {
        return Err(FetchError::Refused(format!(
            "{url} is not on the package's own registry"
        )));
    }
    let response = ARCHIVE_CLIENT
        .get(url)
        .header("User-Agent", eco.user_agent())
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| FetchError::Transient(format!("request failed: {e}")))?;
    let status = response.status();
    if status.is_redirection() {
        // The client stops at a redirect that leaves the archive hosts.
        return Err(FetchError::Refused(format!(
            "{url} redirected off the registry"
        )));
    }
    if status.as_u16() == 404 || status.as_u16() == 410 {
        return Err(FetchError::Refused(format!(
            "registry answered HTTP {status}"
        )));
    }
    if !status.is_success() {
        return Err(FetchError::Transient(format!(
            "registry answered HTTP {status}"
        )));
    }
    Ok(response)
}

/// Read a body, refusing at `cap` bytes instead of buffering whatever the
/// server sends (the declared length is checked first; a server can lie
/// about it, so the bytes actually read are counted too).
async fn read_capped(mut response: reqwest::Response, cap: usize) -> Result<Vec<u8>, FetchError> {
    if let Some(declared) = response.content_length() {
        if declared > cap as u64 {
            return Err(FetchError::Refused(format!(
                "archive is {declared} bytes compressed (cap {cap})"
            )));
        }
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| FetchError::Transient(format!("download interrupted: {e}")))?
    {
        if body.len() + chunk.len() > cap {
            return Err(FetchError::Refused(format!(
                "archive exceeds {cap} bytes compressed"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// The archive URL for one published version.
async fn archive_url(eco: Ecosystem, name: &str, version: &str) -> Result<String, FetchError> {
    match eco {
        Ecosystem::Crates => Ok(crate_archive_url(name, version)),
        Ecosystem::Npm => {
            let response = get(&npm_manifest_url(name, version), eco, MANIFEST_TIMEOUT).await?;
            let manifest: serde_json::Value = response
                .json()
                .await
                .map_err(|e| FetchError::Transient(format!("manifest unreadable: {e}")))?;
            npm_tarball_from_manifest(&manifest).ok_or_else(|| {
                FetchError::Refused(
                    "the version's tarball is not hosted on registry.npmjs.org".into(),
                )
            })
        }
    }
}

/// Download one version's compressed archive (size-capped).
pub(crate) async fn download_archive(
    eco: Ecosystem,
    name: &str,
    version: &str,
) -> Result<Vec<u8>, FetchError> {
    if !is_valid_package_name(name, eco) || !is_valid_version(version) {
        return Err(FetchError::Refused(format!(
            "{name} {version} is not a valid {} package/version",
            eco.as_str()
        )));
    }
    let url = archive_url(eco, name, version).await?;
    let response = get(&url, eco, ARCHIVE_TIMEOUT).await?;
    read_capped(response, MAX_COMPRESSED_BYTES).await
}
