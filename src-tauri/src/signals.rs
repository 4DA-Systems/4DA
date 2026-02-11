use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Signal Types & Priority
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    SecurityAlert,
    BreakingChange,
    ToolDiscovery,
    TechTrend,
    Learning,
    CompetitiveIntel,
}

impl SignalType {
    /// Base priority weight for this signal type (1-4 scale).
    /// Lowered from 4/3 to 2/2 so single-keyword matches don't auto-Critical.
    fn base_weight(&self) -> u8 {
        match self {
            SignalType::SecurityAlert => 2,
            SignalType::BreakingChange => 2,
            SignalType::ToolDiscovery => 2,
            SignalType::TechTrend => 1,
            SignalType::Learning => 1,
            SignalType::CompetitiveIntel => 1,
        }
    }

    /// Snake_case identifier for JSON serialization
    pub fn slug(&self) -> &'static str {
        match self {
            SignalType::SecurityAlert => "security_alert",
            SignalType::BreakingChange => "breaking_change",
            SignalType::ToolDiscovery => "tool_discovery",
            SignalType::TechTrend => "tech_trend",
            SignalType::Learning => "learning",
            SignalType::CompetitiveIntel => "competitive_intel",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            SignalType::SecurityAlert => "Security Alert",
            SignalType::BreakingChange => "Breaking Change",
            SignalType::ToolDiscovery => "Tool Discovery",
            SignalType::TechTrend => "Tech Trend",
            SignalType::Learning => "Learning",
            SignalType::CompetitiveIntel => "Competitive Intel",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SignalPriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl SignalPriority {
    fn from_score(score: u8) -> Self {
        match score {
            4.. => SignalPriority::Critical,
            3 => SignalPriority::High,
            2 => SignalPriority::Medium,
            _ => SignalPriority::Low,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SignalPriority::Critical => "critical",
            SignalPriority::High => "high",
            SignalPriority::Medium => "medium",
            SignalPriority::Low => "low",
        }
    }
}

// ============================================================================
// Classification Result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalClassification {
    pub signal_type: SignalType,
    pub priority: SignalPriority,
    pub confidence: f32,
    pub action: String,
    pub triggers: Vec<String>,
}

// ============================================================================
// Pattern Matching Engine
// ============================================================================

struct SignalPattern {
    keywords: Vec<&'static str>,
    boost_words: Vec<&'static str>,
    weight: f32,
}

pub struct SignalClassifier {
    patterns: HashMap<SignalType, Vec<SignalPattern>>,
}

impl SignalClassifier {
    pub fn new() -> Self {
        let mut patterns: HashMap<SignalType, Vec<SignalPattern>> = HashMap::new();

        patterns.insert(
            SignalType::SecurityAlert,
            vec![SignalPattern {
                keywords: vec![
                    "cve",
                    "vulnerability",
                    "exploit",
                    "breach",
                    "security flaw",
                    "zero-day",
                    "zero day",
                    "0-day",
                    "0day",
                    "patch",
                    "ransomware",
                    "malware",
                    "rce",
                    "injection attack",
                    "xss",
                    "csrf",
                    "privilege escalation",
                    "backdoor",
                    "supply chain attack",
                ],
                boost_words: vec![
                    "critical",
                    "urgent",
                    "severe",
                    "actively exploited",
                    "emergency",
                ],
                weight: 1.0,
            }],
        );

        patterns.insert(
            SignalType::BreakingChange,
            vec![SignalPattern {
                keywords: vec![
                    "breaking change",
                    "deprecated",
                    "end of life",
                    "eol",
                    "migration guide",
                    "major release",
                    "drops support",
                    "removed in",
                    "no longer supported",
                    "sunset",
                    "backwards incompatible",
                    "api change",
                ],
                boost_words: vec!["v2", "v3", "v4", "v5", "major version", "upgrade required"],
                weight: 0.9,
            }],
        );

        patterns.insert(
            SignalType::ToolDiscovery,
            vec![SignalPattern {
                keywords: vec![
                    "new release",
                    "just released",
                    "announcing",
                    "launch",
                    "alternative to",
                    "built with",
                    "replacement for",
                    "open source",
                    "open-source",
                    "introducing",
                    "we built",
                    "i built",
                    "show hn",
                ],
                boost_words: vec!["faster", "better", "simpler", "lightweight", "blazing"],
                weight: 0.7,
            }],
        );

        patterns.insert(
            SignalType::TechTrend,
            vec![SignalPattern {
                keywords: vec![
                    "adoption",
                    "growing",
                    "trending",
                    "benchmark",
                    "comparison",
                    "state of",
                    "survey",
                    "report",
                    "market share",
                    "ecosystem",
                    "roadmap",
                ],
                boost_words: vec!["2025", "2026", "accelerating", "mainstream", "industry"],
                weight: 0.6,
            }],
        );

        patterns.insert(
            SignalType::Learning,
            vec![SignalPattern {
                keywords: vec![
                    "tutorial",
                    "how to",
                    "guide",
                    "deep dive",
                    "explained",
                    "best practices",
                    "patterns",
                    "architecture",
                    "lessons learned",
                    "walkthrough",
                    "step by step",
                    "from scratch",
                ],
                boost_words: vec!["advanced", "production", "real-world", "comprehensive"],
                weight: 0.5,
            }],
        );

        patterns.insert(
            SignalType::CompetitiveIntel,
            vec![SignalPattern {
                keywords: vec![
                    "acquired",
                    "funding",
                    "raised",
                    "ipo",
                    "valuation",
                    "market share",
                    "competitor",
                    "pivots",
                    "pivot",
                    "layoffs",
                    "shutdown",
                    "acqui-hire",
                    "series a",
                    "series b",
                ],
                boost_words: vec!["million", "billion", "disrupts", "overtakes"],
                weight: 0.6,
            }],
        );

        Self { patterns }
    }

    /// Classify an item based on its title, content, relevance score, and ACE tech stack
    pub fn classify(
        &self,
        title: &str,
        content: &str,
        relevance_score: f32,
        ace_tech: &[String],
    ) -> Option<SignalClassification> {
        let text_lower = format!("{} {}", title, content).to_lowercase();
        let title_lower = title.to_lowercase();

        let mut best: Option<(SignalType, f32, Vec<String>)> = None;

        for (signal_type, signal_patterns) in &self.patterns {
            for pattern in signal_patterns {
                let mut matched_keywords: Vec<String> = Vec::new();
                let mut score: f32 = 0.0;

                // Match keywords
                for &kw in &pattern.keywords {
                    if text_lower.contains(kw) {
                        score += pattern.weight;
                        matched_keywords.push(kw.to_string());
                        // Title match is worth more
                        if title_lower.contains(kw) {
                            score += pattern.weight * 0.5;
                        }
                    }
                }

                // Boost words add extra confidence
                for &bw in &pattern.boost_words {
                    if text_lower.contains(bw) {
                        score += 0.2;
                        matched_keywords.push(bw.to_string());
                    }
                }

                if !matched_keywords.is_empty() {
                    // Normalize: cap at 1.0
                    let confidence = (score / 3.0).min(1.0);

                    if let Some((_, ref best_conf, _)) = best {
                        if confidence > *best_conf {
                            best = Some((signal_type.clone(), confidence, matched_keywords));
                        }
                    } else {
                        best = Some((signal_type.clone(), confidence, matched_keywords));
                    }
                }
            }
        }

        let (signal_type, confidence, triggers) = best?;

        // Require at least 2 keyword matches to classify any signal.
        // Single keyword matches produce too many false positives.
        let trigger_count = triggers.len();
        if trigger_count < 2 {
            return None;
        }

        // Bonus for ACE tech stack match
        let tech_match = ace_tech.iter().find(|tech| {
            let t = tech.to_lowercase();
            text_lower.contains(&t)
        });

        // Priority escalation: base weight + bonuses
        // CRITICAL requires both tech match AND high relevance — not just keyword count
        let priority_score = {
            let mut ps = signal_type.base_weight();
            if trigger_count >= 4 {
                ps = ps.saturating_add(1); // 4+ keywords = extra escalation (was 3)
            }
            // Tech match AND high relevance required together for escalation
            if tech_match.is_some() && relevance_score > 0.7 {
                ps = ps.saturating_add(1);
            }
            ps
        };

        let priority = SignalPriority::from_score(priority_score.min(4));

        // Generate action text
        let action = self.generate_action(&signal_type, title, tech_match.map(|s| s.as_str()));

        Some(SignalClassification {
            signal_type,
            priority,
            confidence,
            action,
            triggers,
        })
    }

    fn generate_action(
        &self,
        signal_type: &SignalType,
        title: &str,
        matched_tech: Option<&str>,
    ) -> String {
        let short_title: String = title.chars().take(60).collect();
        match (signal_type, matched_tech) {
            (SignalType::SecurityAlert, Some(tech)) => {
                format!("Review {} - affects your {} stack", short_title, tech)
            }
            (SignalType::SecurityAlert, None) => {
                format!("Review security implications: {}", short_title)
            }
            (SignalType::BreakingChange, Some(tech)) => {
                format!("Check migration path - {} breaking change", tech)
            }
            (SignalType::BreakingChange, None) => {
                format!("Review breaking change: {}", short_title)
            }
            (SignalType::ToolDiscovery, Some(tech)) => {
                format!("Evaluate for your {} workflow: {}", tech, short_title)
            }
            (SignalType::ToolDiscovery, None) => {
                format!("Evaluate new tool: {}", short_title)
            }
            (SignalType::TechTrend, Some(tech)) => {
                format!("Track {} trend: {}", tech, short_title)
            }
            (SignalType::TechTrend, None) => {
                format!("{}: {}", signal_type.label(), short_title)
            }
            (SignalType::Learning, Some(tech)) => {
                format!("Learn - {} resource: {}", tech, short_title)
            }
            (SignalType::Learning, None) => {
                format!("Learning resource: {}", short_title)
            }
            (SignalType::CompetitiveIntel, Some(tech)) => {
                format!("Competitive move in {} space: {}", tech, short_title)
            }
            (SignalType::CompetitiveIntel, None) => {
                format!("{}: {}", signal_type.label(), short_title)
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_alert_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify(
            "Critical CVE-2026-1234 in SQLite",
            "A severe vulnerability has been found in SQLite allowing remote code execution",
            0.8,
            &["sqlite".to_string(), "rust".to_string()],
        );
        let c = result.expect("should classify as security alert");
        assert_eq!(c.signal_type, SignalType::SecurityAlert);
        assert!(c.priority >= SignalPriority::High);
        assert!(c.confidence > 0.0);
        assert!(!c.triggers.is_empty());
        assert!(c.action.contains("sqlite"));
    }

    #[test]
    fn test_breaking_change_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify(
            "React 20 drops class components - migration guide",
            "This major release removes support for class components",
            0.6,
            &["react".to_string()],
        );
        let c = result.expect("should classify as breaking change");
        assert_eq!(c.signal_type, SignalType::BreakingChange);
        assert!(c.action.contains("react") || c.action.contains("React"));
    }

    #[test]
    fn test_tool_discovery_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify(
            "Show HN: A new Rust testing framework - blazing fast alternative to cargo test",
            "We just released a lightweight open source tool",
            0.5,
            &["rust".to_string()],
        );
        let c = result.expect("should classify as tool discovery");
        assert_eq!(c.signal_type, SignalType::ToolDiscovery);
    }

    #[test]
    fn test_learning_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify(
            "Deep dive: Advanced async patterns in Rust explained",
            "A comprehensive tutorial and best practices guide",
            0.4,
            &["rust".to_string()],
        );
        let c = result.expect("should classify as learning");
        assert_eq!(c.signal_type, SignalType::Learning);
    }

    #[test]
    fn test_no_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify("What's your favorite color?", "I like blue", 0.1, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_tech_match_boosts_priority() {
        let classifier = SignalClassifier::new();

        // Without tech match
        let no_match = classifier
            .classify(
                "New vulnerability found in obscure library",
                "A CVE was discovered",
                0.3,
                &["rust".to_string()],
            )
            .unwrap();

        // With tech match
        let with_match = classifier
            .classify(
                "New vulnerability found in Rust standard library",
                "A CVE was discovered in rust",
                0.3,
                &["rust".to_string()],
            )
            .unwrap();

        assert!(with_match.priority >= no_match.priority);
    }

    #[test]
    fn test_high_relevance_boosts_priority() {
        let classifier = SignalClassifier::new();

        let low_score = classifier
            .classify(
                "New tutorial on async patterns",
                "A guide to best practices",
                0.2,
                &[],
            )
            .unwrap();

        let high_score = classifier
            .classify(
                "New tutorial on async patterns",
                "A guide to best practices",
                0.9,
                &[],
            )
            .unwrap();

        assert!(high_score.priority >= low_score.priority);
    }

    #[test]
    fn test_competitive_intel_classification() {
        let classifier = SignalClassifier::new();
        let result = classifier.classify(
            "Electron alternative raises $50M in Series B funding",
            "The company has been acquired for a high valuation",
            0.6,
            &["tauri".to_string()],
        );
        let c = result.expect("should classify as competitive intel");
        assert_eq!(c.signal_type, SignalType::CompetitiveIntel);
    }

    #[test]
    fn test_priority_levels() {
        assert_eq!(SignalPriority::from_score(1), SignalPriority::Low);
        assert_eq!(SignalPriority::from_score(2), SignalPriority::Medium);
        assert_eq!(SignalPriority::from_score(3), SignalPriority::High);
        assert_eq!(SignalPriority::from_score(4), SignalPriority::Critical);
        assert_eq!(SignalPriority::from_score(5), SignalPriority::Critical);
    }
}
