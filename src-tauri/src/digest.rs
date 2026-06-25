// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Email digest system for 4DA
//!
//! Collects relevant items over a time period and formats them into
//! digestible summaries that can be sent via email or saved locally.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{Result, ResultExt};

#[path = "digest_rendering.rs"]
mod digest_rendering;

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
    // REMOVE BY 2026-08-01
    #[allow(dead_code)] // Reason: deserialized from JSON for SMTP auth
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
    /// Signal priority level (e.g., "critical", "alert", "advisory", "watch")
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
            let topic = item.matched_topics.first().map_or_else(
                || {
                    let lang = crate::i18n::get_user_language();
                    crate::i18n::t("ui:digest.topicGeneral", &lang, &[])
                },
                |t| Self::normalize_topic(t),
            );

            groups.entry(topic).or_default().push(item);
        }

        // Sort groups by item count (largest first)
        let mut sorted: Vec<_> = groups.into_iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.1.len()));
        sorted
    }

    /// Normalize topic name for display
    fn normalize_topic(topic: &str) -> String {
        let lang = crate::i18n::get_user_language();
        let key = match topic.to_lowercase().as_str() {
            "rust" | "cargo" => "ui:digest.topicRust",
            "typescript" | "javascript" | "react" | "vue" | "node" => "ui:digest.topicWeb",
            "python" | "django" | "flask" => "ui:digest.topicPython",
            "ai" | "ml" | "llm" | "gpt" | "claude" | "machine learning" => "ui:digest.topicAI",
            "security" | "vulnerability" | "hack" => "ui:digest.topicSecurity",
            "database" | "sql" | "postgresql" | "mongodb" => "ui:digest.topicDatabases",
            "cloud" | "aws" | "gcp" | "azure" | "kubernetes" => "ui:digest.topicCloud",
            _ => return topic.to_string(),
        };
        crate::i18n::t(key, &lang, &[])
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
        top_topics.sort_by_key(|b| std::cmp::Reverse(b.1));
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
}

/// Digest manager handles collection and delivery of digests
pub struct DigestManager {
    pub config: DigestConfig,
}

impl DigestManager {
    pub fn new(config: DigestConfig) -> Self {
        Self { config }
    }

    /// Save digest to local file
    pub fn save_local(&self, digest: &Digest) -> Result<PathBuf> {
        let output_dir = self.config.output_dir.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("4da")
                .join("digests")
        });

        std::fs::create_dir_all(&output_dir).context("Failed to create digest directory")?;

        // Save as markdown
        let md_path = output_dir.join(format!("{}.md", digest.id));
        std::fs::write(&md_path, digest.to_markdown()).context("Failed to save markdown digest")?;

        // Save as HTML
        let html_path = output_dir.join(format!("{}.html", digest.id));
        std::fs::write(&html_path, digest.to_html()).context("Failed to save HTML digest")?;

        Ok(md_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

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
                signal_priority: Some("alert".to_string()),
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

        let lang = crate::i18n::get_user_language();
        let rust_label = crate::i18n::t("ui:digest.topicRust", &lang, &[]);
        let ai_label = crate::i18n::t("ui:digest.topicAI", &lang, &[]);
        let general_label = crate::i18n::t("ui:digest.topicGeneral", &lang, &[]);

        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());
        let groups = digest.group_by_topic();

        // Should have 3 groups: Rust (2), AI/ML (1), General (1)
        assert_eq!(groups.len(), 3);

        // First group should be Rust-related with 2 items (largest group sorts first)
        assert_eq!(groups[0].0, rust_label);
        assert_eq!(groups[0].1.len(), 2);

        // AI/ML and General each have 1 item
        let ai_ml = groups.iter().find(|(t, _)| t == &ai_label);
        assert!(ai_ml.is_some());
        assert_eq!(ai_ml.unwrap().1.len(), 1);

        let general = groups.iter().find(|(t, _)| t == &general_label);
        assert!(general.is_some());
        assert_eq!(general.unwrap().1.len(), 1);
    }

    #[test]
    fn test_normalize_topic() {
        let lang = crate::i18n::get_user_language();
        let rust_dev = crate::i18n::t("ui:digest.topicRust", &lang, &[]);
        let web_dev = crate::i18n::t("ui:digest.topicWeb", &lang, &[]);
        let python = crate::i18n::t("ui:digest.topicPython", &lang, &[]);
        let ai_ml = crate::i18n::t("ui:digest.topicAI", &lang, &[]);
        let security = crate::i18n::t("ui:digest.topicSecurity", &lang, &[]);
        let databases = crate::i18n::t("ui:digest.topicDatabases", &lang, &[]);
        let cloud = crate::i18n::t("ui:digest.topicCloud", &lang, &[]);

        // Rust-related topics all normalize to the same group
        assert_eq!(Digest::normalize_topic("rust"), rust_dev);
        assert_eq!(Digest::normalize_topic("Cargo"), rust_dev);
        // Web-related topics all normalize to the same group
        assert_eq!(Digest::normalize_topic("typescript"), web_dev);
        assert_eq!(Digest::normalize_topic("React"), web_dev);
        // Other category mappings
        assert_eq!(Digest::normalize_topic("PYTHON"), python);
        assert_eq!(Digest::normalize_topic("AI"), ai_ml);
        assert_eq!(Digest::normalize_topic("machine learning"), ai_ml);
        assert_eq!(Digest::normalize_topic("security"), security);
        assert_eq!(Digest::normalize_topic("postgresql"), databases);
        assert_eq!(Digest::normalize_topic("kubernetes"), cloud);
        // Unknown topics pass through unchanged
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
        let lang = crate::i18n::get_user_language();
        let signals_label = crate::i18n::t("ui:digest.signals", &lang, &[]);
        let text = digest.to_text();
        assert!(
            text.contains(&signals_label),
            "Text output should contain signals header"
        );
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
