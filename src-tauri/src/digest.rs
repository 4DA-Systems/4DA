//! Email digest system for 4DA
//!
//! Collects relevant items over a time period and formats them into
//! digestible summaries that can be sent via email or saved locally.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Email digest configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestConfig {
    /// Whether digest is enabled
    pub enabled: bool,
    /// Frequency: "daily", "weekly", "realtime"
    pub frequency: String,
    /// Email address to send digests to (if using email)
    pub email: Option<String>,
    /// SMTP configuration (if sending via email)
    pub smtp: Option<SmtpConfig>,
    /// Save digests to local file
    pub save_local: bool,
    /// Local digest output directory
    pub output_dir: Option<PathBuf>,
    /// Minimum relevance score to include in digest
    pub min_score: f64,
    /// Maximum items per digest
    pub max_items: usize,
    /// Last digest sent timestamp
    pub last_sent: Option<DateTime<Utc>>,
    /// Whether to generate LLM summaries for top items (requires LLM configured)
    #[serde(default)]
    pub generate_summaries: bool,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency: "daily".to_string(),
            email: None,
            smtp: None,
            save_local: true,
            output_dir: None,
            min_score: 0.35,
            max_items: 20,
            last_sent: None,
            generate_summaries: false,
        }
    }
}

/// SMTP configuration for sending emails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing)]
    #[allow(dead_code)] // Deserialized from JSON, used for SMTP auth
    pub password: String,
    pub from_address: String,
    pub use_tls: bool,
}

/// A single item in the digest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestItem {
    pub id: i64,
    pub title: String,
    pub url: Option<String>,
    pub source: String,
    pub relevance_score: f64,
    pub matched_topics: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    /// LLM-generated summary (if enabled)
    #[serde(default)]
    pub summary: Option<String>,
    /// Signal classification type (e.g., "security_alert", "tool_discovery")
    #[serde(default)]
    pub signal_type: Option<String>,
    /// Signal priority level (e.g., "critical", "high", "medium", "low")
    #[serde(default)]
    pub signal_priority: Option<String>,
    /// Suggested action text
    #[serde(default)]
    pub signal_action: Option<String>,
}

/// A compiled digest ready for delivery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Digest {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub items: Vec<DigestItem>,
    pub summary: DigestSummary,
}

/// Summary statistics for a digest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestSummary {
    pub total_items: usize,
    pub sources: HashMap<String, usize>,
    pub top_topics: Vec<(String, usize)>,
    pub avg_relevance: f64,
    /// Count of items by signal type
    #[serde(default)]
    pub signal_counts: HashMap<String, usize>,
    /// Count of critical/high priority signals
    #[serde(default)]
    pub critical_count: usize,
    #[serde(default)]
    pub high_count: usize,
}

impl Digest {
    /// Create a new digest from items
    pub fn new(
        items: Vec<DigestItem>,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Self {
        let summary = Self::compute_summary(&items);
        let id = format!("digest_{}", Utc::now().format("%Y%m%d_%H%M%S"));

        Self {
            id,
            created_at: Utc::now(),
            period_start,
            period_end,
            items,
            summary,
        }
    }

    /// Group items by their primary topic for clustered display
    pub fn group_by_topic(&self) -> Vec<(String, Vec<&DigestItem>)> {
        let mut groups: HashMap<String, Vec<&DigestItem>> = HashMap::new();

        for item in &self.items {
            // Use first matched topic, or "General" if none
            let topic = item
                .matched_topics
                .first()
                .map(|t| Self::normalize_topic(t))
                .unwrap_or_else(|| "General".to_string());

            groups.entry(topic).or_default().push(item);
        }

        // Sort groups by item count (largest first)
        let mut sorted: Vec<_> = groups.into_iter().collect();
        sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        sorted
    }

    /// Normalize topic name for display
    fn normalize_topic(topic: &str) -> String {
        match topic.to_lowercase().as_str() {
            "rust" | "cargo" => "Rust Development".to_string(),
            "typescript" | "javascript" | "react" | "vue" | "node" => "Web Development".to_string(),
            "python" | "django" | "flask" => "Python".to_string(),
            "ai" | "ml" | "llm" | "gpt" | "claude" | "machine learning" => "AI/ML".to_string(),
            "security" | "vulnerability" | "hack" => "Security".to_string(),
            "database" | "sql" | "postgresql" | "mongodb" => "Databases".to_string(),
            "cloud" | "aws" | "gcp" | "azure" | "kubernetes" => "Cloud/DevOps".to_string(),
            _ => topic.to_string(),
        }
    }

    fn compute_summary(items: &[DigestItem]) -> DigestSummary {
        let total_items = items.len();

        // Count by source
        let mut sources: HashMap<String, usize> = HashMap::new();
        for item in items {
            *sources.entry(item.source.clone()).or_insert(0) += 1;
        }

        // Count topics
        let mut topic_counts: HashMap<String, usize> = HashMap::new();
        for item in items {
            for topic in &item.matched_topics {
                *topic_counts.entry(topic.clone()).or_insert(0) += 1;
            }
        }

        // Get top topics
        let mut top_topics: Vec<(String, usize)> = topic_counts.into_iter().collect();
        top_topics.sort_by(|a, b| b.1.cmp(&a.1));
        top_topics.truncate(5);

        // Calculate average relevance
        let avg_relevance = if total_items > 0 {
            items.iter().map(|i| i.relevance_score).sum::<f64>() / total_items as f64
        } else {
            0.0
        };

        // Count signals
        let mut signal_counts: HashMap<String, usize> = HashMap::new();
        let mut critical_count = 0usize;
        let mut high_count = 0usize;
        for item in items {
            if let Some(ref st) = item.signal_type {
                *signal_counts.entry(st.clone()).or_insert(0) += 1;
            }
            match item.signal_priority.as_deref() {
                Some("critical") => critical_count += 1,
                Some("high") => high_count += 1,
                _ => {}
            }
        }

        DigestSummary {
            total_items,
            sources,
            top_topics,
            avg_relevance,
            signal_counts,
            critical_count,
            high_count,
        }
    }

    /// Format digest as plain text
    pub fn to_text(&self) -> String {
        let mut output = String::new();

        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str("                    4DA DIGEST\n");
        output.push_str("           The internet searched for you.\n");
        output.push_str("═══════════════════════════════════════════════════════════════\n\n");

        output.push_str(&format!(
            "Period: {} to {}\n",
            self.period_start.format("%Y-%m-%d %H:%M"),
            self.period_end.format("%Y-%m-%d %H:%M")
        ));
        output.push_str(&format!("Items found: {}\n", self.summary.total_items));
        output.push_str(&format!(
            "Average relevance: {:.1}%\n",
            self.summary.avg_relevance * 100.0
        ));
        output.push('\n');

        // Sources breakdown
        output.push_str("Sources:\n");
        for (source, count) in &self.summary.sources {
            output.push_str(&format!("  - {}: {} items\n", source, count));
        }
        output.push('\n');

        // Top topics
        if !self.summary.top_topics.is_empty() {
            output.push_str("Top Topics:\n");
            for (topic, count) in &self.summary.top_topics {
                output.push_str(&format!("  - {} ({})\n", topic, count));
            }
            output.push('\n');
        }

        // Signals section (only if any signals detected)
        let signal_items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.signal_type.is_some())
            .collect();
        if !signal_items.is_empty() {
            output.push_str("───────────────────────────────────────────────────────────────\n");
            output.push_str("                      SIGNALS\n");
            output.push_str("───────────────────────────────────────────────────────────────\n\n");
            if self.summary.critical_count > 0 {
                output.push_str(&format!(
                    "  !! {} CRITICAL signals require attention\n",
                    self.summary.critical_count
                ));
            }
            if self.summary.high_count > 0 {
                output.push_str(&format!(
                    "  !  {} HIGH priority signals\n",
                    self.summary.high_count
                ));
            }
            output.push('\n');
            for item in &signal_items {
                let priority_marker = match item.signal_priority.as_deref() {
                    Some("critical") => "!!",
                    Some("high") => "! ",
                    _ => "  ",
                };
                let signal_label = match item.signal_type.as_deref() {
                    Some("security_alert") => "SECURITY",
                    Some("breaking_change") => "BREAKING",
                    Some("tool_discovery") => "TOOL",
                    Some("tech_trend") => "TREND",
                    Some("learning") => "LEARN",
                    Some("competitive_intel") => "INTEL",
                    Some(other) => other,
                    None => "SIGNAL",
                };
                output.push_str(&format!(
                    "  {} [{}] {}\n",
                    priority_marker,
                    signal_label,
                    item.signal_action.as_deref().unwrap_or(&item.title)
                ));
            }
            output.push('\n');
        }

        output.push_str("───────────────────────────────────────────────────────────────\n");
        output.push_str("                       ITEMS\n");
        output.push_str("───────────────────────────────────────────────────────────────\n\n");

        for (i, item) in self.items.iter().enumerate() {
            output.push_str(&format!(
                "{}. [{}] {:.1}% - {}\n",
                i + 1,
                item.source,
                item.relevance_score * 100.0,
                item.title
            ));
            if let Some(url) = &item.url {
                output.push_str(&format!("   {}\n", url));
            }
            if !item.matched_topics.is_empty() {
                output.push_str(&format!("   Topics: {}\n", item.matched_topics.join(", ")));
            }
            if let Some(ref action) = item.signal_action {
                output.push_str(&format!("   Signal: {}\n", action));
            }
            output.push('\n');
        }

        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str("Generated by 4DA - https://4da.dev\n");

        output
    }

    /// Format digest as HTML
    pub fn to_html(&self) -> String {
        let mut output = String::new();

        output.push_str(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; background: #0A0A0A; color: #FFFFFF; }
        .header { text-align: center; padding: 20px 0; border-bottom: 1px solid #2A2A2A; }
        .header h1 { margin: 0; font-size: 24px; }
        .header p { margin: 5px 0 0; color: #A0A0A0; font-size: 14px; }
        .summary { background: #141414; padding: 15px; border-radius: 8px; margin: 20px 0; }
        .summary h2 { margin: 0 0 10px; font-size: 14px; color: #A0A0A0; text-transform: uppercase; }
        .stat { display: inline-block; margin-right: 20px; }
        .stat-value { font-size: 20px; font-weight: bold; }
        .stat-label { font-size: 12px; color: #A0A0A0; }
        .item { background: #141414; padding: 15px; border-radius: 8px; margin: 10px 0; border-left: 3px solid #D4AF37; }
        .item-score { display: inline-block; background: #1F1F1F; padding: 2px 8px; border-radius: 4px; font-size: 12px; font-family: monospace; }
        .item-score.high { color: #22C55E; }
        .item-score.medium { color: #D4AF37; }
        .item-score.low { color: #666666; }
        .item-title { font-size: 16px; margin: 8px 0; }
        .item-title a { color: #FFFFFF; text-decoration: none; }
        .item-title a:hover { color: #D4AF37; }
        .item-meta { font-size: 12px; color: #666666; }
        .item-topics { margin-top: 8px; }
        .topic { display: inline-block; background: #1F1F1F; padding: 2px 8px; border-radius: 4px; font-size: 11px; color: #A0A0A0; margin-right: 5px; }
        .footer { text-align: center; padding: 20px 0; color: #666666; font-size: 12px; border-top: 1px solid #2A2A2A; margin-top: 30px; }
        .signals { background: #141414; padding: 15px; border-radius: 8px; margin: 20px 0; border-left: 3px solid #EF4444; }
        .signals h2 { margin: 0 0 10px; font-size: 14px; color: #EF4444; text-transform: uppercase; }
        .signal-item { padding: 8px 0; border-bottom: 1px solid #1F1F1F; }
        .signal-item:last-child { border-bottom: none; }
        .signal-badge { display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 600; }
        .signal-badge.security { background: #EF444420; color: #EF4444; }
        .signal-badge.breaking { background: #F59E0B20; color: #F59E0B; }
        .signal-badge.tool { background: #3B82F620; color: #3B82F6; }
        .signal-badge.trend { background: #8B5CF620; color: #8B5CF6; }
        .signal-badge.learning { background: #22C55E20; color: #22C55E; }
        .signal-badge.intel { background: #06B6D420; color: #06B6D4; }
        .priority-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 6px; }
        .priority-dot.critical { background: #EF4444; }
        .priority-dot.high { background: #F97316; }
        .priority-dot.medium { background: #EAB308; }
        .priority-dot.low { background: #666666; }
    </style>
</head>
<body>
"#);

        output.push_str(
            r#"<div class="header">
    <h1>4DA Digest</h1>
    <p>The internet searched for you.</p>
</div>"#,
        );

        output.push_str(&format!(
            r#"<div class="summary">
    <h2>Summary</h2>
    <div class="stat">
        <div class="stat-value">{}</div>
        <div class="stat-label">Items Found</div>
    </div>
    <div class="stat">
        <div class="stat-value">{:.0}%</div>
        <div class="stat-label">Avg Relevance</div>
    </div>
</div>"#,
            self.summary.total_items,
            self.summary.avg_relevance * 100.0
        ));

        // Signals section
        let html_signal_items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.signal_type.is_some())
            .collect();
        if !html_signal_items.is_empty() {
            output.push_str(r#"<div class="signals"><h2>Actionable Signals</h2>"#);
            for item in &html_signal_items {
                let badge_class = match item.signal_type.as_deref() {
                    Some("security_alert") => "security",
                    Some("breaking_change") => "breaking",
                    Some("tool_discovery") => "tool",
                    Some("tech_trend") => "trend",
                    Some("learning") => "learning",
                    Some("competitive_intel") => "intel",
                    _ => "trend",
                };
                let priority_class = item.signal_priority.as_deref().unwrap_or("low");
                let label = match item.signal_type.as_deref() {
                    Some("security_alert") => "Security",
                    Some("breaking_change") => "Breaking",
                    Some("tool_discovery") => "Tool",
                    Some("tech_trend") => "Trend",
                    Some("learning") => "Learning",
                    Some("competitive_intel") => "Intel",
                    _ => "Signal",
                };
                output.push_str(&format!(
                    r#"<div class="signal-item"><span class="priority-dot {}"></span><span class="signal-badge {}">{}</span> {}</div>"#,
                    priority_class, badge_class, label,
                    item.signal_action.as_deref().unwrap_or(&item.title)
                ));
            }
            output.push_str("</div>");
        }

        for item in &self.items {
            let score_class = if item.relevance_score >= 0.5 {
                "high"
            } else if item.relevance_score >= 0.35 {
                "medium"
            } else {
                "low"
            };

            output.push_str(&format!(
                r#"<div class="item">
    <span class="item-score {}">{:.1}%</span>
    <span class="item-meta"> · {} · {}</span>
    <div class="item-title">"#,
                score_class,
                item.relevance_score * 100.0,
                item.source,
                item.discovered_at.format("%b %d")
            ));

            if let Some(url) = &item.url {
                output.push_str(&format!(r#"<a href="{}">{}</a>"#, url, item.title));
            } else {
                output.push_str(&item.title);
            }

            output.push_str("</div>");

            if !item.matched_topics.is_empty() {
                output.push_str(r#"<div class="item-topics">"#);
                for topic in &item.matched_topics {
                    output.push_str(&format!(r#"<span class="topic">{}</span>"#, topic));
                }
                output.push_str("</div>");
            }

            output.push_str("</div>");
        }

        output.push_str(
            r#"<div class="footer">
    Generated by <a href="https://4da.dev" style="color: #D4AF37;">4DA</a> — All signal. No feed.
</div>
</body>
</html>"#,
        );

        output
    }

    /// Format digest as Markdown
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("# 4DA Digest\n\n");
        output.push_str("*The internet searched for you.*\n\n");
        output.push_str("---\n\n");

        output.push_str("## Summary\n\n");
        output.push_str(&format!(
            "- **Period:** {} to {}\n",
            self.period_start.format("%Y-%m-%d %H:%M"),
            self.period_end.format("%Y-%m-%d %H:%M")
        ));
        output.push_str(&format!(
            "- **Items Found:** {}\n",
            self.summary.total_items
        ));
        output.push_str(&format!(
            "- **Average Relevance:** {:.1}%\n",
            self.summary.avg_relevance * 100.0
        ));
        output.push('\n');

        // Signals section in markdown
        let md_signal_items: Vec<_> = self
            .items
            .iter()
            .filter(|i| i.signal_type.is_some())
            .collect();
        if !md_signal_items.is_empty() {
            output.push_str("## Actionable Signals\n\n");
            if self.summary.critical_count > 0 {
                output.push_str(&format!(
                    "> **{}** critical signal{} require attention\n\n",
                    self.summary.critical_count,
                    if self.summary.critical_count > 1 {
                        "s"
                    } else {
                        ""
                    }
                ));
            }
            for item in &md_signal_items {
                let emoji = match item.signal_type.as_deref() {
                    Some("security_alert") => "🛡",
                    Some("breaking_change") => "⚠",
                    Some("tool_discovery") => "🔧",
                    Some("tech_trend") => "📈",
                    Some("learning") => "📚",
                    Some("competitive_intel") => "🏢",
                    _ => "⚡",
                };
                let priority = item
                    .signal_priority
                    .as_deref()
                    .unwrap_or("low")
                    .to_uppercase();
                let action = item.signal_action.as_deref().unwrap_or(&item.title);
                output.push_str(&format!("- {} **[{}]** {}\n", emoji, priority, action));
            }
            output.push('\n');
        }

        output.push_str("---\n\n");

        // Group items by topic
        let groups = self.group_by_topic();

        for (topic, items) in groups {
            output.push_str(&format!("## {} ({} items)\n\n", topic, items.len()));

            for item in items {
                output.push_str(&format!(
                    "### {} ({:.0}%)\n\n",
                    item.title,
                    item.relevance_score * 100.0
                ));
                output.push_str(&format!(
                    "**Source:** {} | **Found:** {}\n\n",
                    item.source,
                    item.discovered_at.format("%Y-%m-%d")
                ));

                if let Some(url) = &item.url {
                    output.push_str(&format!("**Link:** [{}]({})\n\n", item.title, url));
                }

                if !item.matched_topics.is_empty() {
                    output.push_str(&format!(
                        "**Topics:** {}\n\n",
                        item.matched_topics
                            .iter()
                            .map(|t| format!("`{}`", t))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }

                // Include LLM summary if present
                if let Some(summary) = &item.summary {
                    output.push_str(&format!("**Summary:** {}\n\n", summary));
                }
            }
        }

        output.push_str("---\n\n");
        output.push_str("*Generated by [4DA](https://4da.dev)*\n");

        output
    }
}

/// Digest manager handles collection and delivery of digests
pub struct DigestManager {
    pub config: DigestConfig,
}

impl DigestManager {
    pub fn new(config: DigestConfig) -> Self {
        Self { config }
    }

    /// Check if it's time to send a new digest
    #[allow(dead_code)] // Future: scheduled digest delivery
    pub fn should_send(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        let now = Utc::now();
        let last = self
            .config
            .last_sent
            .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap_or(Utc::now()));

        match self.config.frequency.as_str() {
            "realtime" => true,
            "daily" => now - last >= Duration::hours(24),
            "weekly" => now - last >= Duration::days(7),
            _ => now - last >= Duration::hours(24),
        }
    }

    /// Get the time range for the next digest
    #[allow(dead_code)] // Future: scheduled digest delivery
    pub fn get_digest_period(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let end = Utc::now();
        let start = self.config.last_sent.unwrap_or(end - Duration::hours(24));
        (start, end)
    }

    /// Save digest to local file
    pub fn save_local(&self, digest: &Digest) -> Result<PathBuf, String> {
        let output_dir = self.config.output_dir.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("4da")
                .join("digests")
        });

        std::fs::create_dir_all(&output_dir)
            .map_err(|e| format!("Failed to create digest directory: {}", e))?;

        // Save as markdown
        let md_path = output_dir.join(format!("{}.md", digest.id));
        std::fs::write(&md_path, digest.to_markdown())
            .map_err(|e| format!("Failed to save markdown digest: {}", e))?;

        // Save as HTML
        let html_path = output_dir.join(format!("{}.html", digest.id));
        std::fs::write(&html_path, digest.to_html())
            .map_err(|e| format!("Failed to save HTML digest: {}", e))?;

        Ok(md_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest_creation() {
        let items = vec![
            DigestItem {
                id: 1,
                title: "Test Item 1".to_string(),
                url: Some("https://example.com/1".to_string()),
                source: "hackernews".to_string(),
                relevance_score: 0.75,
                matched_topics: vec!["rust".to_string(), "programming".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: Some("tool_discovery".to_string()),
                signal_priority: Some("high".to_string()),
                signal_action: Some("Evaluate new Rust testing framework".to_string()),
            },
            DigestItem {
                id: 2,
                title: "Test Item 2".to_string(),
                url: None,
                source: "arxiv".to_string(),
                relevance_score: 0.45,
                matched_topics: vec!["machine learning".to_string()],
                discovered_at: Utc::now(),
                summary: Some("An AI/ML paper summary".to_string()),
                signal_type: None,
                signal_priority: None,
                signal_action: None,
            },
        ];

        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());

        assert_eq!(digest.summary.total_items, 2);
        assert!(!digest.to_text().is_empty());
        assert!(!digest.to_html().is_empty());
        assert!(!digest.to_markdown().is_empty());
    }

    #[test]
    fn test_digest_config_default() {
        let config = DigestConfig::default();
        assert!(config.enabled);
        assert_eq!(config.frequency, "daily");
        assert_eq!(config.min_score, 0.35);
        assert!(!config.generate_summaries);
    }

    #[test]
    fn test_group_by_topic() {
        let items = vec![
            DigestItem {
                id: 1,
                title: "Rust Article".to_string(),
                url: None,
                source: "hackernews".to_string(),
                relevance_score: 0.8,
                matched_topics: vec!["rust".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: Some("security_alert".to_string()),
                signal_priority: Some("critical".to_string()),
                signal_action: Some("Review CVE - affects your rust stack".to_string()),
            },
            DigestItem {
                id: 2,
                title: "More Rust".to_string(),
                url: None,
                source: "reddit".to_string(),
                relevance_score: 0.7,
                matched_topics: vec!["cargo".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
            },
            DigestItem {
                id: 3,
                title: "AI Paper".to_string(),
                url: None,
                source: "arxiv".to_string(),
                relevance_score: 0.6,
                matched_topics: vec!["machine learning".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
            },
            DigestItem {
                id: 4,
                title: "No Topic".to_string(),
                url: None,
                source: "hackernews".to_string(),
                relevance_score: 0.5,
                matched_topics: vec![],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
            },
        ];

        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());
        let groups = digest.group_by_topic();

        // Should have 3 groups: Rust Development (2), AI/ML (1), General (1)
        assert_eq!(groups.len(), 3);

        // First group should be Rust Development with 2 items
        assert_eq!(groups[0].0, "Rust Development");
        assert_eq!(groups[0].1.len(), 2);

        // AI/ML and General each have 1 item
        let ai_ml = groups.iter().find(|(t, _)| t == "AI/ML");
        assert!(ai_ml.is_some());
        assert_eq!(ai_ml.unwrap().1.len(), 1);

        let general = groups.iter().find(|(t, _)| t == "General");
        assert!(general.is_some());
        assert_eq!(general.unwrap().1.len(), 1);
    }

    #[test]
    fn test_normalize_topic() {
        assert_eq!(Digest::normalize_topic("rust"), "Rust Development");
        assert_eq!(Digest::normalize_topic("Cargo"), "Rust Development");
        assert_eq!(Digest::normalize_topic("typescript"), "Web Development");
        assert_eq!(Digest::normalize_topic("React"), "Web Development");
        assert_eq!(Digest::normalize_topic("PYTHON"), "Python");
        assert_eq!(Digest::normalize_topic("AI"), "AI/ML");
        assert_eq!(Digest::normalize_topic("machine learning"), "AI/ML");
        assert_eq!(Digest::normalize_topic("security"), "Security");
        assert_eq!(Digest::normalize_topic("postgresql"), "Databases");
        assert_eq!(Digest::normalize_topic("kubernetes"), "Cloud/DevOps");
        assert_eq!(Digest::normalize_topic("custom topic"), "custom topic");
    }

    #[test]
    fn test_digest_signal_summary() {
        let items = vec![
            DigestItem {
                id: 1,
                title: "Critical CVE".to_string(),
                url: None,
                source: "hackernews".to_string(),
                relevance_score: 0.9,
                matched_topics: vec!["security".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: Some("security_alert".to_string()),
                signal_priority: Some("critical".to_string()),
                signal_action: Some("Review CVE - affects your sqlite stack".to_string()),
            },
            DigestItem {
                id: 2,
                title: "New Tool".to_string(),
                url: None,
                source: "hackernews".to_string(),
                relevance_score: 0.7,
                matched_topics: vec!["rust".to_string()],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: Some("tool_discovery".to_string()),
                signal_priority: Some("high".to_string()),
                signal_action: Some("Evaluate new Rust tool".to_string()),
            },
            DigestItem {
                id: 3,
                title: "Regular Item".to_string(),
                url: None,
                source: "arxiv".to_string(),
                relevance_score: 0.5,
                matched_topics: vec![],
                discovered_at: Utc::now(),
                summary: None,
                signal_type: None,
                signal_priority: None,
                signal_action: None,
            },
        ];

        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());

        assert_eq!(digest.summary.critical_count, 1);
        assert_eq!(digest.summary.high_count, 1);
        assert_eq!(digest.summary.signal_counts.len(), 2);
        assert_eq!(digest.summary.signal_counts["security_alert"], 1);
        assert_eq!(digest.summary.signal_counts["tool_discovery"], 1);

        // Verify signals section appears in text output
        let text = digest.to_text();
        assert!(text.contains("SIGNALS"));
        assert!(text.contains("CRITICAL"));
        assert!(text.contains("Review CVE"));

        // Verify signals section appears in markdown
        let md = digest.to_markdown();
        assert!(md.contains("Actionable Signals"));
        assert!(md.contains("Review CVE"));

        // Verify signals section appears in HTML
        let html = digest.to_html();
        assert!(html.contains("Actionable Signals"));
        assert!(html.contains("signal-badge"));
    }
}
