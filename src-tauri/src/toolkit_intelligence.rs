//! Toolkit Intelligence — Backend commands for 4DA-connected intelligence tools
//!
//! Provides three Tauri commands:
//! - `toolkit_test_feed` — Test any RSS/Atom URL and see what 4DA extracts
//! - `toolkit_score_sandbox` — Score a title against your interest profile
//! - `toolkit_generate_export_pack` — Generate shareable developer profile markdown

use serde::{Deserialize, Serialize};
use tracing::info;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTestResult {
    pub feed_title: Option<String>,
    pub format: String,
    pub item_count: usize,
    pub items: Vec<FeedTestItem>,
    pub fetch_duration_ms: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedTestItem {
    pub title: String,
    pub url: String,
    pub published_at: Option<String>,
    pub content_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxScoreResult {
    pub score: f32,
    pub relevant: bool,
    pub breakdown: SandboxBreakdown,
    pub matched_interests: Vec<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxBreakdown {
    pub keyword_score: f32,
    pub interest_score: f32,
    pub ace_boost: f32,
    pub affinity_mult: f32,
    pub domain_relevance: f32,
    pub content_quality: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPackResult {
    pub markdown: String,
    pub has_dna: bool,
    pub has_radar: bool,
    pub has_decisions: bool,
}

// ============================================================================
// Command 1: Test Feed
// ============================================================================

#[tauri::command]
pub async fn toolkit_test_feed(url: String) -> Result<FeedTestResult, String> {
    info!(target: "4da::toolkit", url = %url, "Testing feed URL");

    // Validate URL
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".to_string());
    }

    let client = crate::sources::shared_client();
    let start = std::time::Instant::now();

    // Fetch with timeout
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .header("User-Agent", "4DA/1.0 Feed Tester")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown")
        ));
    }

    let xml = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let fetch_duration_ms = start.elapsed().as_millis() as u64;

    // Detect format
    let format = if (xml.contains("<feed") && xml.contains("xmlns=\"http://www.w3.org/2005/Atom\""))
        || (xml.contains("<entry>") && !xml.contains("<item>"))
    {
        "Atom".to_string()
    } else if xml.contains("<rss") || xml.contains("<item>") {
        "RSS 2.0".to_string()
    } else {
        "Unknown".to_string()
    };

    // Parse using RssSource's parse_feed
    let rss_source = crate::sources::rss::RssSource::new();
    let entries = rss_source.parse_feed(&xml, &url);

    let mut errors = Vec::new();
    if entries.is_empty() && format == "Unknown" {
        errors.push(
            "Could not detect feed format. Ensure the URL points to a valid RSS or Atom feed."
                .to_string(),
        );
    }

    let feed_title = entries.first().map(|e| e.feed_title.clone());
    let item_count = entries.len();

    // Take first 10 items with content preview
    let items: Vec<FeedTestItem> = entries
        .into_iter()
        .take(10)
        .map(|e| {
            let preview = if e.description.len() > 200 {
                let truncated = crate::truncate_utf8(&e.description, 200);
                format!("{}...", truncated)
            } else {
                e.description.clone()
            };
            FeedTestItem {
                title: e.title,
                url: e.link,
                published_at: e.pub_date,
                content_preview: preview,
            }
        })
        .collect();

    info!(target: "4da::toolkit", format = %format, items = item_count, duration_ms = fetch_duration_ms, "Feed test complete");

    Ok(FeedTestResult {
        feed_title,
        format,
        item_count,
        items,
        fetch_duration_ms,
        errors,
    })
}

// ============================================================================
// Command 2: Scoring Sandbox
// ============================================================================

#[tauri::command]
pub async fn toolkit_score_sandbox(
    title: String,
    content: Option<String>,
    source_type: Option<String>,
) -> Result<SandboxScoreResult, String> {
    info!(target: "4da::toolkit", title = %title, "Scoring sandbox request");

    let db = crate::get_database()?;
    let ctx = crate::scoring::build_scoring_context(db).await?;

    let content_str = content.unwrap_or_default();
    let src_type = source_type.unwrap_or_else(|| "sandbox".to_string());
    let empty_embedding: Vec<f32> = vec![];

    let input = crate::scoring::ScoringInput {
        id: 0,
        title: &title,
        url: None,
        content: &content_str,
        source_type: &src_type,
        embedding: &empty_embedding,
        created_at: None,
    };

    let options = crate::scoring::ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
    };

    let result = crate::scoring::score_item(&input, &ctx, db, &options, None);

    // Extract breakdown details
    let breakdown = if let Some(ref bd) = result.score_breakdown {
        SandboxBreakdown {
            keyword_score: bd.keyword_score,
            interest_score: bd.interest_score,
            ace_boost: bd.ace_boost,
            affinity_mult: bd.affinity_mult,
            domain_relevance: bd.context_score,
            content_quality: bd.source_quality_boost,
        }
    } else {
        SandboxBreakdown {
            keyword_score: 0.0,
            interest_score: result.interest_score,
            ace_boost: 0.0,
            affinity_mult: 1.0,
            domain_relevance: result.context_score,
            content_quality: 0.0,
        }
    };

    // Extract matched interests from matches
    let matched_interests: Vec<String> = result
        .matches
        .iter()
        .map(|m| m.matched_text.clone())
        .collect();

    info!(target: "4da::toolkit", score = result.top_score, relevant = result.relevant, "Sandbox scoring complete");

    Ok(SandboxScoreResult {
        score: result.top_score,
        relevant: result.relevant,
        breakdown,
        matched_interests,
        explanation: result.explanation,
    })
}

// ============================================================================
// Command 3: Export Pack
// ============================================================================

#[tauri::command]
pub async fn toolkit_generate_export_pack() -> Result<ExportPackResult, String> {
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
                            s.push_str(&format!("**Rationale:** {}\n\n", rationale));
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
