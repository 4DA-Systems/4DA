//! Toolkit Export Pack — Generates a shareable developer profile markdown document
//!
//! Extracted from `toolkit_intelligence.rs` to keep modules under the 1000-line limit.
//! Provides `toolkit_generate_export_pack` Tauri command.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::Result;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPackResult {
    pub markdown: String,
    pub has_dna: bool,
    pub has_radar: bool,
    pub has_decisions: bool,
}

// ============================================================================
// Command: Export Pack
// ============================================================================

#[tauri::command]
pub async fn toolkit_generate_export_pack() -> Result<ExportPackResult> {
    info!(target: "4da::toolkit", "Generating export pack");

    let mut sections = Vec::new();
    let mut has_dna = false;
    let mut has_radar = false;
    let mut has_decisions = false;

    // Section 1: Developer DNA
    match crate::developer_dna::generate_dna() {
        Ok(dna) => {
            has_dna = true;
            let mut s = String::new();
            s.push_str("## Developer DNA\n\n");
            s.push_str(&format!("**Identity:** {}\n\n", dna.identity_summary));

            if !dna.primary_stack.is_empty() {
                s.push_str(&format!(
                    "**Primary Stack:** {}\n\n",
                    dna.primary_stack.join(", ")
                ));
            }
            if !dna.adjacent_tech.is_empty() {
                s.push_str(&format!(
                    "**Adjacent Tech:** {}\n\n",
                    dna.adjacent_tech.join(", ")
                ));
            }
            if !dna.interests.is_empty() {
                s.push_str(&format!("**Interests:** {}\n\n", dna.interests.join(", ")));
            }

            // Stats
            s.push_str("### Stats\n\n");
            s.push_str("| Metric | Value |\n|--------|-------|\n");
            s.push_str(&format!(
                "| Items Processed | {} |\n",
                dna.stats.total_items_processed
            ));
            s.push_str(&format!(
                "| Items Relevant | {} |\n",
                dna.stats.total_relevant
            ));
            s.push_str(&format!(
                "| Rejection Rate | {:.1}% |\n",
                dna.stats.rejection_rate
            ));
            s.push_str(&format!(
                "| Projects Monitored | {} |\n",
                dna.stats.project_count
            ));
            s.push_str(&format!(
                "| Dependencies Tracked | {} |\n",
                dna.stats.dependency_count
            ));

            // Top engaged topics
            if !dna.top_engaged_topics.is_empty() {
                s.push_str("\n### Top Engaged Topics\n\n");
                for topic in &dna.top_engaged_topics {
                    s.push_str(&format!(
                        "- **{}** — {} interactions ({:.0}%)\n",
                        topic.topic, topic.interactions, topic.percent_of_total
                    ));
                }
            }

            // Blind spots
            if !dna.blind_spots.is_empty() {
                s.push_str("\n### Knowledge Blind Spots\n\n");
                for spot in &dna.blind_spots {
                    s.push_str(&format!(
                        "- **{}** — {} severity, {} days stale\n",
                        spot.dependency, spot.severity, spot.days_stale
                    ));
                }
            }

            s.push('\n');
            sections.push(s);
        }
        Err(e) => {
            info!(target: "4da::toolkit", error = %e, "DNA generation skipped");
        }
    }

    // Section 2: Tech Radar
    match crate::open_db_connection() {
        Ok(conn) => match crate::tech_radar::compute_radar(&conn) {
            Ok(radar) => {
                if !radar.entries.is_empty() {
                    has_radar = true;
                    let mut s = String::new();
                    s.push_str("## Tech Radar\n\n");
                    s.push_str("| Technology | Ring | Quadrant | Movement | Score |\n");
                    s.push_str("|------------|------|----------|----------|-------|\n");

                    for entry in &radar.entries {
                        let ring = format!("{:?}", entry.ring);
                        let quadrant = format!("{:?}", entry.quadrant);
                        let movement = match &entry.movement {
                            crate::tech_radar::RadarMovement::Up => "^",
                            crate::tech_radar::RadarMovement::Down => "v",
                            crate::tech_radar::RadarMovement::Stable => "-",
                            crate::tech_radar::RadarMovement::New => "*",
                        };
                        s.push_str(&format!(
                            "| {} | {} | {} | {} | {:.2} |\n",
                            entry.name, ring, quadrant, movement, entry.score
                        ));
                    }
                    s.push('\n');
                    sections.push(s);
                }
            }
            Err(e) => {
                info!(target: "4da::toolkit", error = %e, "Radar generation skipped");
            }
        },
        Err(e) => {
            info!(target: "4da::toolkit", error = %e, "DB connection for radar failed");
        }
    }

    // Section 3: Decisions
    match crate::open_db_connection() {
        Ok(conn) => match crate::decisions::list_decisions(&conn, None, None, 50) {
            Ok(decisions) => {
                if !decisions.is_empty() {
                    has_decisions = true;
                    let mut s = String::new();
                    s.push_str("## Active Decisions\n\n");

                    for d in &decisions {
                        s.push_str(&format!(
                            "### {} ({})\n\n",
                            d.subject,
                            d.decision_type.as_str()
                        ));
                        s.push_str(&format!("**Decision:** {}\n\n", d.decision));
                        if let Some(ref rationale) = d.rationale {
                            s.push_str(&format!("**Rationale:** {rationale}\n\n"));
                        }
                        if !d.alternatives_rejected.is_empty() {
                            s.push_str(&format!(
                                "**Alternatives rejected:** {}\n\n",
                                d.alternatives_rejected.join(", ")
                            ));
                        }
                        s.push_str(&format!(
                            "**Confidence:** {:.0}% | **Status:** {:?} | **Updated:** {}\n\n",
                            d.confidence * 100.0,
                            d.status,
                            d.updated_at
                        ));
                        s.push_str("---\n\n");
                    }
                    sections.push(s);
                }
            }
            Err(e) => {
                info!(target: "4da::toolkit", error = %e, "Decision listing skipped");
            }
        },
        Err(e) => {
            info!(target: "4da::toolkit", error = %e, "DB connection for decisions failed");
        }
    }

    // Assemble final markdown
    let mut markdown = String::new();
    markdown.push_str("# 4DA Developer Profile\n\n");
    markdown.push_str(&format!(
        "*Generated: {}*\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));

    if sections.is_empty() {
        markdown
            .push_str("No profile data available yet. Use 4DA to build your developer profile.\n");
    } else {
        for section in &sections {
            markdown.push_str(section);
        }
    }

    markdown.push_str(
        "\n---\n*Generated by [4DA](https://4da.dev) — Private Developer Intelligence*\n",
    );

    info!(target: "4da::toolkit",
        has_dna, has_radar, has_decisions,
        sections = sections.len(),
        "Export pack generated"
    );

    Ok(ExportPackResult {
        markdown,
        has_dna,
        has_radar,
        has_decisions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_pack_result_serde_roundtrip() {
        let pack = ExportPackResult {
            markdown: "# 4DA Developer Profile\n\nTest content\n".into(),
            has_dna: true,
            has_radar: false,
            has_decisions: true,
        };

        let json = serde_json::to_string(&pack).expect("serialize");
        let decoded: ExportPackResult = serde_json::from_str(&json).expect("deserialize");

        assert!(decoded.markdown.starts_with("# 4DA Developer Profile"));
        assert!(decoded.has_dna);
        assert!(!decoded.has_radar);
        assert!(decoded.has_decisions);
    }
}
