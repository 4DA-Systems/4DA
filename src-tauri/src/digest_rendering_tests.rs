// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The HTML digest is built from third-party text (audit 2026-11-11): every
//! title, source name, topic and action is escaped, and only a plain web
//! address becomes a link. The text and Markdown digests print the same text
//! as it is.

use chrono::{Duration, Utc};

use super::super::{Digest, DigestItem};
use super::{escape_html, web_href};

fn item(title: &str, url: Option<&str>) -> DigestItem {
    DigestItem {
        id: 1,
        title: title.to_string(),
        url: url.map(str::to_string),
        source: "rss".to_string(),
        relevance_score: 0.8,
        matched_topics: vec![],
        discovered_at: Utc::now(),
        summary: None,
        signal_type: None,
        signal_priority: None,
        signal_action: None,
    }
}

fn digest(items: Vec<DigestItem>) -> Digest {
    Digest::new(items, Utc::now() - Duration::hours(24), Utc::now())
}

/// Markup a feed can put in any text field, and what it must read as.
const HOSTILE: &str = r#"<img src=x onerror="alert(1)">&'"#;
const INERT: &str = "&lt;img src=x onerror=&quot;alert(1)&quot;&gt;&amp;&#39;";

#[test]
fn html_digest_escapes_third_party_text_everywhere_it_prints_it() {
    let mut hostile = item(&format!("title {HOSTILE}"), Some("https://example.com/a"));
    hostile.source = format!("source {HOSTILE}");
    hostile.matched_topics = vec![format!("topic {HOSTILE}")];
    hostile.signal_type = Some("security_alert".to_string());
    hostile.signal_priority = Some(r#"critical" onmouseover="alert(2)"#.to_string());
    hostile.signal_action = Some(format!("action {HOSTILE}"));
    // A second item with no link and no action: its title prints twice, as
    // the signal line's fallback and as the unlinked item title.
    let mut unlinked = item(&format!("unlinked {HOSTILE}"), None);
    unlinked.signal_type = Some("tool_discovery".to_string());

    let html = digest(vec![hostile, unlinked]).to_html();

    assert!(
        !html.contains("<img"),
        "markup from an item reached the page"
    );
    assert!(!html.contains(r#"onerror=""#) && !html.contains("onmouseover"));
    for field in ["title", "source", "topic", "action"] {
        assert!(
            html.contains(&format!("{field} {INERT}")),
            "the {field} is printed escaped"
        );
    }
    assert_eq!(
        html.matches(&format!("unlinked {INERT}")).count(),
        2,
        "the signal line's title fallback and the unlinked title are both escaped"
    );
    // The priority is a class from a fixed set, never the stored string.
    assert!(html.contains(r#"<span class="priority-dot watch">"#));
}

#[test]
fn html_digest_links_only_plain_web_addresses() {
    let linked = [
        (
            "https://example.com/a?b=1&c=2",
            r#"<a href="https://example.com/a?b=1&amp;c=2">Linked title</a>"#,
        ),
        (
            "HTTP://Example.com/x",
            r#"<a href="http://example.com/x">Linked title</a>"#,
        ),
    ];
    for (url, expected) in linked {
        let html = digest(vec![item("Linked title", Some(url))]).to_html();
        assert!(html.contains(expected), "{url} -> {expected}");
    }

    let not_linked = [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        " java\nscript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "file:///C:/Windows/System32/calc.exe",
        "vbscript:msgbox(1)",
        "/relative/path",
        "",
    ];
    for url in not_linked {
        let html = digest(vec![item("Linked title", Some(url))]).to_html();
        assert!(
            html.contains(r#"<div class="item-title">Linked title</div>"#),
            "{url:?} must drop the link and keep the title as text"
        );
        assert!(!html.contains("<script>"), "{url:?}");
    }

    // A web address cannot break out of the attribute either.
    let breakout = r#"https://example.com/"><script>alert(1)</script>"#;
    let html = digest(vec![item("Linked title", Some(breakout))]).to_html();
    assert!(
        !html.contains("<script>") && !html.contains(r#"/">"#),
        "{html}"
    );
    assert!(html.contains(r#"<a href="https://example.com/%22%3E%3Cscript%3E"#));
}

/// The escaping is for HTML only: the text and Markdown digests keep the
/// item's own characters (an `&amp;` in a plain-text email would be a bug).
#[test]
fn text_and_markdown_digests_are_not_html_escaped() {
    let title = r#"Ampersands & <angles> "quotes""#;
    let url = "https://example.com/a?b=1&c=2";
    let d = digest(vec![item(title, Some(url))]);
    for rendered in [d.to_text(), d.to_markdown()] {
        assert!(rendered.contains(title), "{rendered}");
        assert!(rendered.contains(url), "{rendered}");
        assert!(!rendered.contains("&amp;") && !rendered.contains("&lt;"));
    }
}

#[test]
fn escape_html_covers_text_and_both_attribute_quotes() {
    assert_eq!(
        escape_html(r#"a & b < c > d " e ' f"#),
        "a &amp; b &lt; c &gt; d &quot; e &#39; f"
    );
    assert_eq!(
        escape_html("plain text, ünïcödé ✓"),
        "plain text, ünïcödé ✓"
    );
    // Already-escaped text is escaped again, never trusted.
    assert_eq!(escape_html("&lt;b&gt;"), "&amp;lt;b&amp;gt;");
}

#[test]
fn web_href_admits_http_and_https_only() {
    assert_eq!(
        web_href(" https://example.com/p?q=1&r=2 ").as_deref(),
        Some("https://example.com/p?q=1&amp;r=2")
    );
    assert_eq!(
        web_href("http://example.com").as_deref(),
        Some("http://example.com/")
    );
    for url in [
        "javascript:alert(1)",
        "data:text/html,x",
        "file:///etc/passwd",
        "ftp://example.com/x",
        "mailto:a@example.com",
        "example.com",
        "",
    ] {
        assert_eq!(web_href(url), None, "{url:?}");
    }
}
