/// STREETS Revenue Engine mapping
///
/// Maps scored items to STREETS revenue engines based on content type,
/// signal classification, and keyword patterns.
///
/// Engines:
/// 1. Digital Products — templates, starter kits, guides
/// 2. Content — tutorials, courses, newsletters
/// 3. Micro-SaaS — small web apps, tools
/// 4. Automation — bots, scrapers, workflow tools
/// 5. API Products — APIs, data services
/// 6. Consulting — expertise, advisory
/// 7. Open Source+ — OSS with premium tiers
/// 8. Data Products — datasets, analytics, ML models
///
/// Map a scored item to a STREETS revenue engine based on its properties.
/// Returns None if no clear engine match is detected.
pub(crate) fn map_to_streets_engine(
    title: &str,
    content: &str,
    content_type: Option<&str>,
    signal_type: Option<&str>,
) -> Option<String> {
    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();

    // Signal-type based mapping (highest confidence)
    if let Some(sig) = signal_type {
        let engine = match sig {
            "security_alert" => Some("Engine 4: Automation"),
            "breaking_change" => Some("Engine 2: Content"),
            "tool_discovery" => Some("Engine 1: Digital Products"),
            "tech_trend" => Some("Engine 2: Content"),
            "competitive_intel" => Some("Engine 6: Consulting"),
            _ => None,
        };
        if engine.is_some() {
            return engine.map(|s| s.to_string());
        }
    }

    // Content-type based mapping
    if let Some(ct) = content_type {
        let engine = match ct {
            "security_advisory" => Some("Engine 4: Automation"),
            "show_and_tell" => Some("Engine 7: Open Source+"),
            "tool_release" => Some("Engine 1: Digital Products"),
            "tutorial" => Some("Engine 2: Content"),
            "deep_technical" => Some("Engine 6: Consulting"),
            "opinion" | "discussion" => None, // Too broad to map
            _ => None,
        };
        if engine.is_some() {
            return engine.map(|s| s.to_string());
        }
    }

    // Keyword-based mapping (lower confidence, check title + first 500 chars of content)
    let check_text = format!(
        "{} {}",
        title_lower,
        if content_lower.len() > 500 {
            &content_lower[..500]
        } else {
            &content_lower
        }
    );

    // API / data service patterns
    if has_any(
        &check_text,
        &[
            "api",
            "endpoint",
            "rest api",
            "graphql",
            "webhook",
            "rate limit",
        ],
    ) {
        return Some("Engine 5: API Products".to_string());
    }

    // ML / AI / data patterns
    if has_any(
        &check_text,
        &[
            "machine learning",
            "neural network",
            "training data",
            "fine-tun",
            "llm",
            "model weights",
            "dataset",
        ],
    ) {
        return Some("Engine 5: API Products".to_string());
    }

    // SaaS / web app patterns
    if has_any(
        &check_text,
        &[
            "saas",
            "subscription",
            "monthly recurring",
            "mrr",
            "stripe integration",
            "billing",
            "waitlist",
        ],
    ) {
        return Some("Engine 3: Micro-SaaS".to_string());
    }

    // Automation patterns
    if has_any(
        &check_text,
        &[
            "automat",
            "cron job",
            "scheduled",
            "scraper",
            "bot",
            "pipeline",
            "ci/cd",
            "workflow",
        ],
    ) {
        return Some("Engine 4: Automation".to_string());
    }

    // Open source patterns
    if has_any(
        &check_text,
        &[
            "open source",
            "open-source",
            "github release",
            "npm publish",
            "crates.io",
            "mit license",
            "apache license",
        ],
    ) {
        return Some("Engine 7: Open Source+".to_string());
    }

    // Template / starter kit patterns
    if has_any(
        &check_text,
        &[
            "template",
            "starter kit",
            "boilerplate",
            "scaffold",
            "starter",
            "blueprint",
        ],
    ) {
        return Some("Engine 1: Digital Products".to_string());
    }

    // Tutorial / educational patterns
    if has_any(
        &check_text,
        &[
            "tutorial",
            "how to",
            "step by step",
            "beginner",
            "guide",
            "course",
            "learn",
        ],
    ) {
        return Some("Engine 2: Content".to_string());
    }

    None
}

/// Check if text contains any of the given patterns
fn has_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_type_mapping() {
        assert_eq!(
            map_to_streets_engine("CVE in OpenSSL", "", None, Some("security_alert")),
            Some("Engine 4: Automation".to_string())
        );
        assert_eq!(
            map_to_streets_engine("React 20 released", "", None, Some("breaking_change")),
            Some("Engine 2: Content".to_string())
        );
    }

    #[test]
    fn test_content_type_mapping() {
        assert_eq!(
            map_to_streets_engine("I built X", "", Some("show_and_tell"), None),
            Some("Engine 7: Open Source+".to_string())
        );
        assert_eq!(
            map_to_streets_engine("New CLI tool", "", Some("tool_release"), None),
            Some("Engine 1: Digital Products".to_string())
        );
    }

    #[test]
    fn test_keyword_mapping() {
        assert_eq!(
            map_to_streets_engine("Building a REST API with Rust", "api endpoint", None, None),
            Some("Engine 5: API Products".to_string())
        );
        assert_eq!(
            map_to_streets_engine("My SaaS hit $1k MRR", "subscription billing", None, None),
            Some("Engine 3: Micro-SaaS".to_string())
        );
    }

    #[test]
    fn test_no_match_returns_none() {
        assert_eq!(
            map_to_streets_engine("Random news about cats", "meow", None, None),
            None
        );
    }
}
