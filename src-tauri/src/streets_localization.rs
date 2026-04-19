// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! STREETS Content Localization — region-specific prices and context for STREETS playbook.
//!
//! Loads regional data from `docs/streets/regions/*.json` at runtime,
//! keeping the binary small and allowing users to add their own regional data.
//! Exchange rates are static (updated quarterly) — privacy first, no live API.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionalData {
    pub country: String,
    pub currency: String,
    pub currency_symbol: String,
    pub electricity_kwh: f64,
    pub internet_typical_monthly: f64,
    pub business_registration_cost: f64,
    pub business_entity_type: String,
    pub tax_note: String,
    pub payment_processors: Vec<String>,
    pub bank_recommendation: String,
    pub isp_note: String,
}

// ============================================================================
// Static Exchange Rates (USD-based, updated quarterly — no live API)
// ============================================================================

fn usd_exchange_rate(currency: &str) -> f64 {
    match currency {
        "USD" => 1.0,
        "EUR" => 0.92,
        "GBP" => 0.79,
        "CAD" => 1.36,
        "AUD" => 1.53,
        "JPY" => 149.5,
        "INR" => 83.0,
        "BRL" => 4.97,
        "CHF" => 0.88,
        "SEK" => 10.4,
        "NOK" => 10.5,
        "DKK" => 6.87,
        "NZD" => 1.63,
        "KRW" => 1320.0,
        "SGD" => 1.34,
        "MXN" => 17.2,
        "CNY" => 7.24,
        _ => 1.0,
    }
}

fn currency_symbol(currency: &str) -> String {
    match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "INR" => "₹",
        "BRL" => "R$",
        "KRW" => "₩",
        "CHF" => "CHF ",
        "CNY" => "¥",
        "CAD" => "C$",
        "AUD" => "A$",
        "NZD" => "NZ$",
        "SEK" => "kr ",
        "NOK" => "kr ",
        "DKK" => "kr ",
        "SGD" => "S$",
        "MXN" => "MX$",
        _ => "$",
    }
    .to_string()
}

// ============================================================================
// Regional Data Loading
// ============================================================================

fn load_regional_file(country_code: &str) -> Option<RegionalData> {
    let regions_dir = crate::runtime_paths::RuntimePaths::get().streets_regions_dir();
    let paths_to_try = vec![
        // Primary: via centralized RuntimePaths
        Some(regions_dir.join(format!("{country_code}.json"))),
        // Fallback: relative to current working directory
        Some(std::path::PathBuf::from("docs/streets/regions").join(format!("{country_code}.json"))),
    ];

    for path in paths_to_try.into_iter().flatten() {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<RegionalData>(&content) {
                    Ok(data) => {
                        debug!(
                            target: "4da::locale",
                            country = country_code,
                            path = %path.display(),
                            "Loaded regional data"
                        );
                        return Some(data);
                    }
                    Err(e) => {
                        warn!(
                            target: "4da::locale",
                            country = country_code,
                            error = %e,
                            "Failed to parse regional data file"
                        );
                    }
                },
                Err(e) => {
                    warn!(
                        target: "4da::locale",
                        country = country_code,
                        error = %e,
                        "Failed to read regional data file"
                    );
                }
            }
        }
    }
    None
}

fn default_regional_data() -> RegionalData {
    RegionalData {
        country: "US".into(),
        currency: "USD".into(),
        currency_symbol: "$".into(),
        electricity_kwh: 0.16,
        internet_typical_monthly: 70.0,
        business_registration_cost: 500.0,
        business_entity_type: "LLC".into(),
        tax_note: "Consult a local tax professional".into(),
        payment_processors: vec!["Stripe".into(), "PayPal".into()],
        bank_recommendation: "Check local business banking options".into(),
        isp_note: "Check local ISP availability".into(),
    }
}

// ============================================================================
// Public Sync Accessors (for internal use by content_personalization)
// ============================================================================

/// Load regional data by country code — sync, no Tauri command overhead.
/// Falls back to default.json, then hardcoded US defaults.
pub(crate) fn load_regional_data_for_context(country_code: &str) -> Option<RegionalData> {
    load_regional_file(&country_code.to_lowercase()).or_else(|| load_regional_file("default"))
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the regional data for the user's configured locale.
/// Falls back to default.json, then hardcoded US defaults.
#[tauri::command]
pub async fn get_regional_data() -> Result<RegionalData> {
    let country = {
        let sm = crate::get_settings_manager().lock();
        sm.get().locale.country.clone()
    };

    let data = load_regional_file(&country.to_lowercase())
        .or_else(|| load_regional_file("default"))
        .unwrap_or_else(default_regional_data);

    info!(
        target: "4da::locale",
        country = %data.country,
        currency = %data.currency,
        "Returning regional data"
    );

    Ok(data)
}

/// Convert a USD amount to the user's configured currency and format it.
/// Returns a formatted string like "€92.00" or "¥14,950".
#[tauri::command]
pub async fn format_currency(amount: f64) -> Result<String> {
    let (currency, symbol) = {
        let sm = crate::get_settings_manager().lock();
        let locale = &sm.get().locale;
        (locale.currency.clone(), currency_symbol(&locale.currency))
    };

    let rate = usd_exchange_rate(&currency);
    let converted = amount * rate;

    // Zero-decimal currencies get no fractional digits
    if ["JPY", "KRW"].contains(&currency.as_str()) {
        Ok(format!("{symbol}{converted:.0}"))
    } else {
        Ok(format!("{symbol}{converted:.2}"))
    }
}

/// Calculate electricity cost for running hardware at a given wattage.
/// Returns daily, monthly, and yearly costs in the user's local currency.
#[tauri::command]
pub async fn calculate_electricity_cost(
    watts: f64,
    hours_per_day: f64,
) -> Result<serde_json::Value> {
    let regional = get_regional_data().await?;
    let kwh_per_day = (watts / 1000.0) * hours_per_day;
    let daily_cost = kwh_per_day * regional.electricity_kwh;
    let monthly_cost = daily_cost * 30.0;
    let yearly_cost = daily_cost * 365.0;

    Ok(serde_json::json!({
        "kwh_per_day": format!("{:.2}", kwh_per_day),
        "daily_cost": format!("{}{:.2}", regional.currency_symbol, daily_cost),
        "monthly_cost": format!("{}{:.2}", regional.currency_symbol, monthly_cost),
        "yearly_cost": format!("{}{:.2}", regional.currency_symbol, yearly_cost),
        "rate_per_kwh": format!("{}{:.3}/kWh", regional.currency_symbol, regional.electricity_kwh),
        "currency": regional.currency,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usd_exchange_rates() {
        assert_eq!(usd_exchange_rate("USD"), 1.0);
        assert!(usd_exchange_rate("EUR") < 1.0);
        assert!(usd_exchange_rate("JPY") > 100.0);
        // Unknown currency defaults to 1.0
        assert_eq!(usd_exchange_rate("XYZ"), 1.0);
    }

    #[test]
    fn test_currency_symbol() {
        assert_eq!(currency_symbol("USD"), "$");
        assert_eq!(currency_symbol("EUR"), "€");
        assert_eq!(currency_symbol("GBP"), "£");
        assert_eq!(currency_symbol("JPY"), "¥");
        assert_eq!(currency_symbol("INR"), "₹");
        assert_eq!(currency_symbol("BRL"), "R$");
        // Unknown falls back to $
        assert_eq!(currency_symbol("XYZ"), "$");
    }

    #[test]
    fn test_default_regional_data() {
        let data = default_regional_data();
        assert_eq!(data.country, "US");
        assert_eq!(data.currency, "USD");
        assert_eq!(data.currency_symbol, "$");
        assert!(data.electricity_kwh > 0.0);
        assert!(!data.payment_processors.is_empty());
    }

    #[test]
    fn test_load_regional_file_us() {
        // This test requires the JSON files to exist at the expected path
        let data = load_regional_file("us");
        if let Some(d) = data {
            assert_eq!(d.country, "US");
            assert_eq!(d.currency, "USD");
            assert_eq!(d.currency_symbol, "$");
        }
        // Not asserting Some — file may not exist in CI
    }

    #[test]
    fn test_load_regional_file_nonexistent() {
        let data = load_regional_file("zz_nonexistent");
        assert!(data.is_none());
    }
}
