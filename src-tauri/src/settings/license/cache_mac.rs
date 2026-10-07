// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Integrity MAC for the Keygen validation cache (audit 2026-10-07).
//!
//! `license_cache.json` is the proof that keeps a Keygen-format key on a paid
//! tier while offline (90-day window). It was plain JSON: writing
//! `{"tier":"signal","validated_at":"2099-…","key_hash":"<sha256 of any key>"}`
//! granted a paid tier for as long as the file stayed put.
//!
//! Every cache written now carries
//! `mac = HMAC-SHA256(machine key, validated_at|tier|key_hash)`, keyed by a
//! 32-byte machine-scope secret held only in the OS credential store
//! ([`crate::settings::keystore::LICENSE_CACHE_MAC_KEY`]). A cache without a valid MAC is never
//! trusted for a PAID tier offline — the next online validation re-signs it.
//! (A cache recording `free` needs no MAC: it can only take access away.)
//!
//! Residual limit, stated plainly: code running as the same user can read the
//! credential store too. This raises the bar from "edit a JSON file" to
//! "extract a credential and re-implement the MAC", which is the point.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
#[cfg(not(test))]
use tracing::warn;

use super::keygen::KeygenValidationCache;
#[cfg(not(test))]
use super::keystore;

type HmacSha256 = Hmac<Sha256>;

/// The bytes the MAC covers.
fn mac_input(cache: &KeygenValidationCache) -> String {
    format!("{}|{}|{}", cache.validated_at, cache.tier, cache.key_hash)
}

/// Hex MAC of `cache` under `key`.
pub(crate) fn compute_mac(key: &[u8; 32], cache: &KeygenValidationCache) -> String {
    let Ok(mut mac) = HmacSha256::new_from_slice(key) else {
        return String::new();
    };
    mac.update(mac_input(cache).as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time check of `cache.mac` against `key`. Missing/garbled → false.
pub(crate) fn mac_matches(key: &[u8; 32], cache: &KeygenValidationCache) -> bool {
    let Some(tag) = cache.mac.as_deref().and_then(|h| hex::decode(h).ok()) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(key) else {
        return false;
    };
    mac.update(mac_input(cache).as_bytes());
    mac.verify_slice(&tag).is_ok()
}

fn parse_key(hex_str: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_str.trim()).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

/// Tests use a fixed key: hermetic, deterministic, and never touches a
/// credential store (hosts without one would otherwise fail paid-cache tests).
#[cfg(test)]
pub(crate) fn machine_mac_key() -> Option<[u8; 32]> {
    Some([0x4D; 32])
}

/// The machine MAC key: loaded from the credential store, generated and stored
/// on first use. Cached once found; a transient failure is retried next call.
/// `None` when no credential store can hold it — caches then stay unsigned and
/// are not trusted for a paid tier offline.
#[cfg(not(test))]
pub(crate) fn machine_mac_key() -> Option<[u8; 32]> {
    static KEY: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    if let Some(k) = KEY.get() {
        return Some(*k);
    }
    let key = load_or_create_key()?;
    Some(*KEY.get_or_init(|| key))
}

#[cfg(not(test))]
fn load_or_create_key() -> Option<[u8; 32]> {
    if let Ok(Some(existing)) = keystore::get_secret(keystore::LICENSE_CACHE_MAC_KEY) {
        if let Some(k) = parse_key(&existing) {
            return Some(k);
        }
        warn!(target: "4da::license", "Stored licence-cache MAC key is malformed — generating a new one");
    }
    let mut seed = [0u8; 32];
    if getrandom::fill(&mut seed).is_err() {
        warn!(target: "4da::license", "No CSPRNG available — licence cache stays unsigned");
        return None;
    }
    match keystore::store_secret(keystore::LICENSE_CACHE_MAC_KEY, &hex::encode(seed)) {
        Ok(true) => Some(seed),
        _ => {
            warn!(target: "4da::license", "Credential store cannot hold the licence-cache MAC key — offline paid-tier cache will not be trusted");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(tier: &str) -> KeygenValidationCache {
        KeygenValidationCache {
            validated_at: "2026-10-01T00:00:00+00:00".into(),
            tier: tier.into(),
            key_hash: "ab".repeat(32),
            mac: None,
        }
    }

    #[test]
    fn mac_round_trips_and_detects_tampering() {
        let key = [7u8; 32];
        let mut c = cache("signal");
        c.mac = Some(compute_mac(&key, &c));
        assert!(mac_matches(&key, &c));

        let mut tier_edit = c.clone();
        tier_edit.tier = "enterprise".into();
        assert!(
            !mac_matches(&key, &tier_edit),
            "tier edit must break the MAC"
        );

        let mut time_edit = c.clone();
        time_edit.validated_at = "2099-01-01T00:00:00+00:00".into();
        assert!(
            !mac_matches(&key, &time_edit),
            "timestamp edit must break the MAC"
        );

        assert!(
            !mac_matches(&[8u8; 32], &c),
            "another machine's key must not verify"
        );
    }

    #[test]
    fn missing_or_garbage_mac_never_verifies() {
        let key = [7u8; 32];
        let mut c = cache("signal");
        assert!(!mac_matches(&key, &c));
        c.mac = Some("not-hex".into());
        assert!(!mac_matches(&key, &c));
        c.mac = Some(String::new());
        assert!(!mac_matches(&key, &c));
    }

    #[test]
    fn parses_only_32_byte_hex_keys() {
        assert!(parse_key(&"00".repeat(32)).is_some());
        assert!(parse_key(&"00".repeat(31)).is_none());
        assert!(parse_key("zz").is_none());
    }
}
