// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Standalone helper functions for settings
//!
//! Locale detection, currency mapping, and other utilities
//! not tied to a specific struct.

use super::types::{LocaleConfig, Settings};

/// Parse a BCP 47 culture name ("en-US", "de-DE", "zh-Hans-CN") into a
/// locale: language = first subtag, country = last 2-letter alphabetic subtag.
pub(crate) fn locale_from_culture_name(culture: &str) -> Option<LocaleConfig> {
    let mut parts = culture.trim().split(['-', '_']);
    let language = parts.next()?.to_lowercase();
    let country = parts
        .rfind(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))?
        .to_uppercase();
    if language.is_empty() {
        return None;
    }
    let currency = country_to_currency(&country);
    Some(LocaleConfig {
        country,
        language,
        currency,
    })
}

/// The user's Windows locale name (`HKCU\Control Panel\International\LocaleName`,
/// e.g. "en-US") — the same value `GetUserDefaultLocaleName` returns, read
/// through the registry so no extra `windows-sys` feature is needed and no
/// PowerShell process is spawned (that took ~1.9 s per launch).
#[cfg(target_os = "windows")]
fn windows_locale_name() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Control Panel\\International")
        .ok()?
        .get_value::<String, _>("LocaleName")
        .ok()
}

/// First-launch locale detection, run exactly once per install.
///
/// When `settings.locale_detected` is unset: if the locale is still the US
/// defaults, `detect` runs and a non-US result is applied; either way the flag
/// is set so it never runs again (an explicit later choice of en-US is never
/// overridden). Returns true when the settings changed and must be persisted.
pub(crate) fn detect_locale_once(
    settings: &mut Settings,
    detect: impl FnOnce() -> LocaleConfig,
) -> bool {
    if settings.locale_detected {
        return false;
    }
    let at_defaults = settings.locale.country == "US"
        && settings.locale.language == "en"
        && settings.locale.currency == "USD";
    if at_defaults {
        let detected = detect();
        if detected.country != "US" || detected.language != "en" {
            tracing::info!(target: "4da::settings", country = %detected.country, language = %detected.language, currency = %detected.currency, "Auto-detected system locale");
            settings.locale = detected;
        }
    }
    settings.locale_detected = true;
    true
}

/// Detect system locale from OS environment
pub fn detect_system_locale() -> LocaleConfig {
    // On Windows, LANG/LC_ALL env vars typically don't exist.
    #[cfg(target_os = "windows")]
    {
        if let Some(locale) = windows_locale_name().and_then(|n| locale_from_culture_name(&n)) {
            return locale;
        }
    }

    // Unix: try LANG/LC_ALL env vars (e.g., "en_US.UTF-8")
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();

    // Parse "en_US.UTF-8" -> country=US, language=en
    if let Some((language, rest)) = lang.split_once('_') {
        let country = rest.split('.').next().unwrap_or("US").to_uppercase();
        let language = language.to_lowercase();
        let currency = country_to_currency(&country);
        return LocaleConfig {
            country,
            language,
            currency,
        };
    }

    // macOS: LANG/LC_ALL often not set. Use 'defaults read' to get system language.
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
        {
            let locale_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // AppleLocale format: "en_US", "ja_JP", "fr_FR", etc.
            if let Some(lang) = locale_str.split('_').next() {
                if !lang.is_empty() && lang.len() == 2 {
                    let country = locale_str.split('_').nth(1).unwrap_or("US").to_uppercase();
                    let language = lang.to_lowercase();
                    let currency = country_to_currency(&country);
                    return LocaleConfig {
                        country,
                        language,
                        currency,
                    };
                }
            }
        }
    }

    LocaleConfig::default()
}

pub(crate) fn country_to_currency(country: &str) -> String {
    match country {
        "US" => "USD",
        "GB" => "GBP",
        "DE" | "FR" | "NL" | "IT" | "ES" | "AT" | "BE" | "FI" | "IE" | "PT" => "EUR",
        "CA" => "CAD",
        "AU" => "AUD",
        "JP" => "JPY",
        "IN" => "INR",
        "BR" => "BRL",
        "CH" => "CHF",
        "SE" => "SEK",
        "NO" => "NOK",
        "DK" => "DKK",
        "NZ" => "NZD",
        "KR" => "KRW",
        "CN" => "CNY",
        "SG" => "SGD",
        "MX" => "MXN",
        _ => "USD",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn de() -> LocaleConfig {
        locale_from_culture_name("de-DE").expect("de-DE parses")
    }

    #[test]
    fn culture_names_parse() {
        let l = de();
        assert_eq!(
            (l.language.as_str(), l.country.as_str(), l.currency.as_str()),
            ("de", "DE", "EUR")
        );
        let zh = locale_from_culture_name("zh-Hans-CN").expect("script subtag");
        assert_eq!((zh.language.as_str(), zh.country.as_str()), ("zh", "CN"));
        assert!(locale_from_culture_name("en").is_none());
        assert!(locale_from_culture_name("").is_none());
    }

    #[test]
    fn detection_runs_once_then_never_again() {
        let calls = Cell::new(0);
        let mut settings = Settings::default();

        // en-US user: detection runs, finds en-US, changes nothing but the flag.
        let changed = detect_locale_once(&mut settings, || {
            calls.set(calls.get() + 1);
            LocaleConfig::default()
        });
        assert!(changed, "the flag must be persisted");
        assert!(settings.locale_detected);
        assert_eq!(calls.get(), 1);

        // Every later launch: no detection at all.
        for _ in 0..3 {
            assert!(!detect_locale_once(&mut settings, || {
                calls.set(calls.get() + 1);
                de()
            }));
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(settings.locale.country, "US");
    }

    #[test]
    fn first_detection_applies_a_non_us_locale() {
        let mut settings = Settings::default();
        assert!(detect_locale_once(&mut settings, de));
        assert_eq!(settings.locale.country, "DE");
        assert_eq!(settings.locale.currency, "EUR");
    }

    #[test]
    fn an_explicit_locale_is_never_detected_over() {
        let mut settings = Settings {
            locale: de(),
            ..Settings::default()
        };
        let called = Cell::new(false);
        let changed = detect_locale_once(&mut settings, || {
            called.set(true);
            LocaleConfig::default()
        });
        assert!(
            !called.get(),
            "an explicit locale must not trigger detection"
        );
        assert!(changed && settings.locale_detected);
        assert_eq!(settings.locale.country, "DE");
    }

    #[test]
    fn the_flag_is_persisted_by_the_first_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = super::super::SettingsManager::new_without_keychain(tmp.path());
        assert!(first.get().locale_detected);
        let on_disk: Settings = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("settings.json")).expect("persisted"),
        )
        .expect("parses");
        assert!(on_disk.locale_detected, "flag must survive a restart");
    }
}
