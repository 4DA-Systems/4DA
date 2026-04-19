// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Standalone helper functions for settings
//!
//! Locale detection, currency mapping, and other utilities
//! not tied to a specific struct.

use super::types::LocaleConfig;

/// Detect system locale from OS environment
pub fn detect_system_locale() -> LocaleConfig {
    // On Windows, LANG/LC_ALL env vars typically don't exist.
    // Use PowerShell to query the system culture (e.g., "en-US", "de-DE").
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-Culture).Name",
            ])
            .output()
        {
            let culture = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // culture is like "en-US", "de-DE", "ja-JP"
            if let Some((lang, country_raw)) = culture.split_once('-') {
                let country = country_raw.to_uppercase();
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
