// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Digest rendering — plain-text, HTML, and Markdown formatters for [`Digest`].

use super::{Digest, DigestItem};

impl Digest {
    /// Format digest as plain text
    // Text rendering: used by email digest delivery
    pub fn to_text(&self) -> String {
        let lang = crate::i18n::get_user_language();
        let mut output = String::new();

        let digest_title = crate::i18n::t("ui:digest.title", &lang, &[]);
        let digest_subtitle = crate::i18n::t("ui:digest.subtitle", &lang, &[]);
        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str(&format!("                    {digest_title}\n"));
        output.push_str(&format!("           {digest_subtitle}\n"));
        output.push_str("═══════════════════════════════════════════════════════════════\n\n");

        output.push_str(&format!(
            "Period: {} to {}\n",
            self.period_start.format("%Y-%m-%d %H:%M"),
            self.period_end.format("%Y-%m-%d %H:%M")
        ));
        let items_found_label = crate::i18n::t("ui:digest.itemsFound", &lang, &[]);
        output.push_str(&format!(
            "{items_found_label} {}\n",
            self.summary.total_items
        ));
        let avg_rel_label = crate::i18n::t("ui:digest.avgRelevance", &lang, &[]);
        output.push_str(&format!(
            "{avg_rel_label} {:.1}%\n",
            self.summary.avg_relevance * 100.0
        ));
        output.push('\n');

        // Sources breakdown
        let sources_label = crate::i18n::t("ui:digest.sources", &lang, &[]);
        output.push_str(&format!("{sources_label}\n"));
        for (source, count) in &self.summary.sources {
            output.push_str(&format!("  - {source}: {count} items\n"));
        }
        output.push('\n');

        // Top topics
        if !self.summary.top_topics.is_empty() {
            let topics_label = crate::i18n::t("ui:digest.topTopics", &lang, &[]);
            output.push_str(&format!("{topics_label}\n"));
            for (topic, count) in &self.summary.top_topics {
                output.push_str(&format!("  - {topic} ({count})\n"));
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
            let signals_label = crate::i18n::t("ui:digest.signals", &lang, &[]);
            output.push_str("───────────────────────────────────────────────────────────────\n");
            output.push_str(&format!("                      {signals_label}\n"));
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
                let signal_label = signal_type_label(item);
                output.push_str(&format!(
                    "  {} [{}] {}\n",
                    priority_marker,
                    signal_label,
                    item.signal_action.as_deref().unwrap_or(&item.title)
                ));
            }
            output.push('\n');
        }

        let items_header = crate::i18n::t("ui:digest.itemsSection", &lang, &[]);
        output.push_str("───────────────────────────────────────────────────────────────\n");
        output.push_str(&format!("                       {items_header}\n"));
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
                output.push_str(&format!("   {url}\n"));
            }
            if !item.matched_topics.is_empty() {
                output.push_str(&format!("   Topics: {}\n", item.matched_topics.join(", ")));
            }
            if let Some(ref action) = item.signal_action {
                output.push_str(&format!("   Signal: {action}\n"));
            }
            output.push('\n');
        }

        let footer = crate::i18n::t("ui:digest.footer", &lang, &[]);
        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str(&format!("{footer}\n"));

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
                let priority_class = item.signal_priority.as_deref().unwrap_or("watch");
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
                    output.push_str(&format!(r#"<span class="topic">{topic}</span>"#));
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
                output.push_str(&format!("- {emoji} **[{priority}]** {action}\n"));
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
                            .map(|t| format!("`{t}`"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }

                // Include LLM summary if present
                if let Some(summary) = &item.summary {
                    output.push_str(&format!("**Summary:** {summary}\n\n"));
                }
            }
        }

        output.push_str("---\n\n");
        output.push_str("*Generated by [4DA](https://4da.dev)*\n");

        output
    }
}

/// Map a signal type to its short display label for text rendering
fn signal_type_label(item: &DigestItem) -> &'static str {
    match item.signal_type.as_deref() {
        Some("security_alert") => "SECURITY",
        Some("breaking_change") => "BREAKING",
        Some("tool_discovery") => "TOOL",
        Some("tech_trend") => "TREND",
        Some("learning") => "LEARN",
        Some("competitive_intel") => "INTEL",
        Some(_) | None => "SIGNAL",
    }
}
