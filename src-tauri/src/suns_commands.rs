// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Suns module -- global registry and helpers.
//!
//! Command functions were removed (not registered in invoke_handler).
//! The `SunRegistry` is ticked from `monitoring.rs` via `get_sun_registry()`.

use once_cell::sync::Lazy;
use parking_lot::Mutex;

// ============================================================================
// Global Sun Registry
// ============================================================================

static SUN_REGISTRY: Lazy<Mutex<crate::suns::SunRegistry>> =
    Lazy::new(|| Mutex::new(crate::suns::SunRegistry::new()));

pub fn get_sun_registry() -> parking_lot::MutexGuard<'static, crate::suns::SunRegistry> {
    SUN_REGISTRY.lock()
}

#[cfg(test)]
mod tests {
    use crate::suns::{SunAlert, SunResult, SunStatus};

    // ---- SunResult construction & serialization ----

    #[test]
    fn test_sun_result_success() {
        let result = SunResult {
            success: true,
            message: "All checks passed".to_string(),
            data: None,
        };
        assert!(result.success);
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["success"], true);
        assert_eq!(json["message"], "All checks passed");
        assert!(json["data"].is_null());
    }

    #[test]
    fn test_sun_result_failure_with_data() {
        let data = serde_json::json!({"error_code": 42, "detail": "timeout"});
        let result = SunResult {
            success: false,
            message: "Check failed".to_string(),
            data: Some(data.clone()),
        };
        assert!(!result.success);
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["data"]["error_code"], 42);
    }

    // ---- SunStatus construction & serialization ----

    #[test]
    fn test_sun_status_construction() {
        let status = SunStatus {
            id: "hardware_monitor".to_string(),
            name: "Hardware Monitor".to_string(),
            module_id: "S".to_string(),
            enabled: true,
            interval_secs: 86400,
            last_run: Some("2025-12-01 10:00:00".to_string()),
            next_run_in_secs: Some(3600),
            last_result: Some("OK".to_string()),
            run_count: 10,
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert_eq!(json["id"], "hardware_monitor");
        assert_eq!(json["module_id"], "S");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["interval_secs"], 86400);
        assert_eq!(json["run_count"], 10);
    }

    #[test]
    fn test_sun_status_with_none_fields() {
        let status = SunStatus {
            id: "test_sun".to_string(),
            name: "Test Sun".to_string(),
            module_id: "T".to_string(),
            enabled: false,
            interval_secs: 3600,
            last_run: None,
            next_run_in_secs: None,
            last_result: None,
            run_count: 0,
        };
        let json = serde_json::to_value(&status).expect("serialize");
        assert!(json["last_run"].is_null());
        assert!(json["next_run_in_secs"].is_null());
        assert!(json["last_result"].is_null());
    }

    // ---- SunAlert construction & serialization ----

    #[test]
    fn test_sun_alert_serialization() {
        let alert = SunAlert {
            id: 1,
            sun_id: "price_tracker".to_string(),
            alert_type: "warning".to_string(),
            message: "Price change detected".to_string(),
            acknowledged: false,
            created_at: "2025-12-01 12:00:00".to_string(),
        };
        let json = serde_json::to_value(&alert).expect("serialize");
        assert_eq!(json["id"], 1);
        assert_eq!(json["sun_id"], "price_tracker");
        assert_eq!(json["acknowledged"], false);
    }

    #[test]
    fn test_sun_alert_deserialization() {
        let json_str = r#"{
            "id": 5,
            "sun_id": "uptime_monitor",
            "alert_type": "critical",
            "message": "Service down",
            "acknowledged": true,
            "created_at": "2025-12-01 14:00:00"
        }"#;
        let alert: SunAlert = serde_json::from_str(json_str).expect("deserialize");
        assert_eq!(alert.id, 5);
        assert_eq!(alert.sun_id, "uptime_monitor");
        assert!(alert.acknowledged);
    }
}
