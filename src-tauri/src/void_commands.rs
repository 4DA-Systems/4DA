// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Void Engine Tauri commands for heartbeat signal visualization.

use crate::error::Result;
use crate::void_engine;
use crate::{get_database, get_monitoring_state};

// ============================================================================
// Void Engine Commands
// ============================================================================

/// Get the current void signal state (for initial mount)
#[tauri::command]
pub fn get_void_signal() -> Result<void_engine::VoidSignal> {
    let db = get_database()?;
    let monitoring = get_monitoring_state();
    Ok(void_engine::compute_signal(db, monitoring))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::void_engine::VoidSignal;

    #[test]
    fn test_void_signal_default() {
        let signal = VoidSignal::default();
        assert_eq!(signal.pulse, 0.0);
        assert_eq!(signal.heat, 0.0);
        assert_eq!(signal.staleness, 1.0);
        assert_eq!(signal.item_count, 0);
        assert_eq!(signal.critical_count, 0);
    }

    #[test]
    fn test_void_signal_serialization_roundtrip() {
        let signal = VoidSignal {
            pulse: 0.8,
            heat: 0.6,
            burst: 0.3,
            morph: 0.1,
            error: 0.0,
            staleness: 0.2,
            item_count: 150,
            signal_intensity: 0.75,
            signal_urgency: 0.5,
            critical_count: 1,
            signal_color_shift: 0.4,
            metabolism: 0.9,
            open_windows: 3,
            advantage_trend: 0.2,
        };
        let json = serde_json::to_string(&signal).expect("serialize");
        let parsed: VoidSignal = serde_json::from_str(&json).expect("deserialize");
        assert!((parsed.pulse - 0.8).abs() < f32::EPSILON);
        assert_eq!(parsed.item_count, 150);
        assert_eq!(parsed.open_windows, 3);
    }
}
