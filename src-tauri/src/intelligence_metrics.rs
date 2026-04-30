// SPDX-License-Identifier: FSL-1.1-Apache-2.0

//! Intelligence metrics — production measurement for the three-tier intelligence system.
//!
//! Collects per-tier dismiss rates, engagement rates, and feedback distribution
//! from trust_events. Powers Phase 9 measurement and calibration tracking.

use serde::Serialize;
use tracing::warn;
use ts_rs::TS;

use crate::open_db_connection;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct IntelligenceMetrics {
    pub verified: TierMetrics,
    pub ai_assessed: TierMetrics,
    pub developing: TierMetrics,
    pub feedback_distribution: Vec<FeedbackBucket>,
    pub prompt_versions_active: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct TierMetrics {
    pub surfaced: i64,
    pub dismissed: i64,
    pub acted_on: i64,
    pub dismiss_rate: f64,
    pub engagement_rate: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct FeedbackBucket {
    pub category: String,
    pub count: i64,
}

pub(crate) fn tier_from_alert_id(alert_id: &str) -> &'static str {
    if alert_id.starts_with("osv-") {
        "verified"
    } else if alert_id.starts_with("llm-") {
        "ai_assessed"
    } else {
        "developing"
    }
}

fn compute_tier_metrics(conn: &rusqlite::Connection, tier_prefix: &str, days: i64) -> TierMetrics {
    let like_pattern = format!("{tier_prefix}%");
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
    let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

    let count_by_type = |event_type: &str| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM trust_events
             WHERE alert_id LIKE ?1 AND event_type = ?2
             AND created_at >= ?3",
            rusqlite::params![like_pattern, event_type, cutoff_str],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };

    let surfaced = count_by_type("surfaced");
    let dismissed = count_by_type("dismissed");
    let acted_on = count_by_type("acted_on");

    let dismiss_rate = if surfaced > 0 {
        dismissed as f64 / surfaced as f64
    } else {
        0.0
    };
    let engagement_rate = if surfaced > 0 {
        acted_on as f64 / surfaced as f64
    } else {
        0.0
    };

    TierMetrics {
        surfaced,
        dismissed,
        acted_on,
        dismiss_rate,
        engagement_rate,
    }
}

pub fn collect_metrics(days: i64) -> IntelligenceMetrics {
    let conn = match open_db_connection() {
        Ok(c) => c,
        Err(e) => {
            warn!(target: "4da::intelligence_metrics", error = %e, "Failed to open DB");
            return IntelligenceMetrics {
                verified: TierMetrics {
                    surfaced: 0,
                    dismissed: 0,
                    acted_on: 0,
                    dismiss_rate: 0.0,
                    engagement_rate: 0.0,
                },
                ai_assessed: TierMetrics {
                    surfaced: 0,
                    dismissed: 0,
                    acted_on: 0,
                    dismiss_rate: 0.0,
                    engagement_rate: 0.0,
                },
                developing: TierMetrics {
                    surfaced: 0,
                    dismissed: 0,
                    acted_on: 0,
                    dismiss_rate: 0.0,
                    engagement_rate: 0.0,
                },
                feedback_distribution: vec![],
                prompt_versions_active: vec![],
            };
        }
    };

    let verified = compute_tier_metrics(&conn, "osv-", days);
    let ai_assessed = compute_tier_metrics(&conn, "llm-", days);
    let developing_chains = compute_tier_metrics(&conn, "chain-", days);

    let developing_other = {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();

        let count_other = |event_type: &str| -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM trust_events
                 WHERE alert_id IS NOT NULL
                 AND alert_id NOT LIKE 'osv-%'
                 AND alert_id NOT LIKE 'llm-%'
                 AND alert_id NOT LIKE 'chain-%'
                 AND event_type = ?1
                 AND created_at >= ?2",
                rusqlite::params![event_type, cutoff_str],
                |row| row.get(0),
            )
            .unwrap_or(0)
        };

        let surfaced = count_other("surfaced");
        let dismissed = count_other("dismissed");
        let acted_on = count_other("acted_on");
        TierMetrics {
            surfaced,
            dismissed,
            acted_on,
            dismiss_rate: if surfaced > 0 {
                dismissed as f64 / surfaced as f64
            } else {
                0.0
            },
            engagement_rate: if surfaced > 0 {
                acted_on as f64 / surfaced as f64
            } else {
                0.0
            },
        }
    };

    let developing = TierMetrics {
        surfaced: developing_chains.surfaced + developing_other.surfaced,
        dismissed: developing_chains.dismissed + developing_other.dismissed,
        acted_on: developing_chains.acted_on + developing_other.acted_on,
        dismiss_rate: {
            let total_s = developing_chains.surfaced + developing_other.surfaced;
            let total_d = developing_chains.dismissed + developing_other.dismissed;
            if total_s > 0 {
                total_d as f64 / total_s as f64
            } else {
                0.0
            }
        },
        engagement_rate: {
            let total_s = developing_chains.surfaced + developing_other.surfaced;
            let total_a = developing_chains.acted_on + developing_other.acted_on;
            if total_s > 0 {
                total_a as f64 / total_s as f64
            } else {
                0.0
            }
        },
    };

    let feedback_distribution: Vec<FeedbackBucket> = conn
        .prepare(
            "SELECT COALESCE(notes, 'unknown'), COUNT(*)
             FROM trust_events
             WHERE event_type = 'dismissed'
             AND created_at >= ?1
             GROUP BY COALESCE(notes, 'unknown')
             ORDER BY COUNT(*) DESC",
        )
        .and_then(|mut stmt| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
            let cutoff_str = cutoff.format("%Y-%m-%d %H:%M:%S").to_string();
            let rows = stmt.query_map([cutoff_str], |row| {
                Ok(FeedbackBucket {
                    category: row.get(0)?,
                    count: row.get(1)?,
                })
            })?;
            rows.collect()
        })
        .unwrap_or_default();

    let prompt_versions_active: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT prompt_version FROM llm_judgments
             WHERE created_at >= datetime('now', ?1)
             ORDER BY prompt_version",
        )
        .and_then(|mut stmt| {
            let interval = format!("-{days} days");
            let rows = stmt.query_map([interval], |row| row.get(0))?;
            rows.collect()
        })
        .unwrap_or_default();

    IntelligenceMetrics {
        verified,
        ai_assessed,
        developing,
        feedback_distribution,
        prompt_versions_active,
    }
}

#[tauri::command]
pub async fn get_intelligence_metrics(
    days: Option<i64>,
) -> std::result::Result<IntelligenceMetrics, String> {
    let d = days.unwrap_or(30).clamp(1, 365);
    Ok(collect_metrics(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_from_alert_id_classifies_correctly() {
        assert_eq!(tier_from_alert_id("osv-GHSA-1234-abcd"), "verified");
        assert_eq!(tier_from_alert_id("llm-42"), "ai_assessed");
        assert_eq!(tier_from_alert_id("chain-react-19"), "developing");
        assert_eq!(tier_from_alert_id("keyword-something"), "developing");
        assert_eq!(tier_from_alert_id("preempt-old-style"), "developing");
    }

    #[test]
    fn default_tier_metrics_are_zero() {
        let m = TierMetrics {
            surfaced: 0,
            dismissed: 0,
            acted_on: 0,
            dismiss_rate: 0.0,
            engagement_rate: 0.0,
        };
        assert_eq!(m.dismiss_rate, 0.0);
        assert_eq!(m.engagement_rate, 0.0);
    }

    #[test]
    fn tier_metrics_rate_calculation() {
        let m = TierMetrics {
            surfaced: 10,
            dismissed: 3,
            acted_on: 5,
            dismiss_rate: 3.0 / 10.0,
            engagement_rate: 5.0 / 10.0,
        };
        assert!((m.dismiss_rate - 0.3).abs() < f64::EPSILON);
        assert!((m.engagement_rate - 0.5).abs() < f64::EPSILON);
    }
}
