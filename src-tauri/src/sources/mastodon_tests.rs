// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the Mastodon source adapter and its title derivation.
use super::*;

#[test]
fn derives_clean_title_from_html() {
    assert_eq!(
        derive_title("<p>Could this <em>kill</em> the project?</p>"),
        "Could this kill the project?"
    );
    assert_eq!(derive_title("<p>a &amp; b &lt; c</p>"), "a & b < c");
    let long = format!("<p>{}</p>", "word ".repeat(60));
    assert!(
        derive_title(&long).ends_with('…'),
        "long titles truncate at a word boundary"
    );
    assert_eq!(derive_title("<p></p>"), "");
}

#[test]
fn first_paragraph_becomes_the_title_when_it_is_a_headline() {
    // Live corpus 2026-08-12, id=17141. The old shape welded the headline
    // onto the body and then cut at 120: "…Supply Chain Attack A large-scale…".
    let html = "<p>Self-Propagating ChainDrop Worm Infects More Than 400 npm Packages in Major Software Supply Chain Attack</p>\
                <p>A large-scale software supply chain attack compromised more than 400 packages.</p>";
    assert_eq!(
        derive_title(html),
        "Self-Propagating ChainDrop Worm Infects More Than 400 npm Packages in Major Software Supply Chain Attack"
    );
    assert!(!derive_title(html).contains('…'));
}

#[test]
fn same_story_posted_twice_derives_one_title() {
    // ids 16136 / 15274: identical headline, different second paragraph.
    // Welding them produced two titles de-duplication could not collapse.
    let head = "<p>DeadLock ransomware: Breaking down a Rust-based encryptor with decentralized recovery infrastructure</p>";
    let a = derive_title(&format!(
        "{head}<p>DeadLock is an emerging ransomware operation first seen in…</p>"
    ));
    let b = derive_title(&format!(
        "{head}<p>Indicators extracted from public reporting. Sources below.</p>"
    ));
    assert_eq!(a, b);
    assert_eq!(
        a,
        "DeadLock ransomware: Breaking down a Rust-based encryptor with decentralized recovery infrastructure"
    );
}

#[test]
fn a_short_first_paragraph_never_hijacks_the_title() {
    // id=14775: "<p>New.</p><p>Microsoft: DeadLock ransomware…</p>". A one-word
    // lead-in is not a headline — the whole body is used, as before.
    let t = derive_title(
        "<p>New.</p><p>Microsoft: DeadLock ransomware: Breaking down a Rust-based encryptor</p>",
    );
    assert!(t.starts_with("New. Microsoft:"), "{t}");
    // Digest headers ("📢 New updates · 3 Aug 9") are the same failure shape.
    let d = derive_title(
        "<p>New updates · 3 Aug 9</p><p>MeiliSearch v1.52.0 — added live task streaming and improved search speed</p>",
    );
    assert!(
        d.contains("MeiliSearch"),
        "digest header swallowed the body: {d}"
    );
}

#[test]
fn a_lead_in_paragraph_never_hijacks_the_title() {
    // id=1013: "Lots of new stuff in pip 26.2:" introduces the list that
    // follows — a trailing connector means the paragraph does not stand alone.
    let t = derive_title(
        "<p>Lots of new stuff in pip 26.2 for everyone:</p><p>Support for Python 3.15 and experimental venv builds</p>",
    );
    assert!(t.contains("Python 3.15"), "lead-in swallowed the body: {t}");
}

#[test]
fn security_id_is_hoisted_from_a_later_paragraph() {
    // Detection reads the whole body, so an id below the fold still leads.
    let t = derive_title(
        "<p>Patch your servers this week, everyone, seriously</p><p>Details in CVE-2026-49975 published today</p>",
    );
    assert!(t.starts_with("CVE-2026-49975 — "), "{t}");
}

#[test]
fn single_paragraph_posts_are_unchanged() {
    assert_eq!(
        derive_title("<p>Could this kill the project?</p>"),
        "Could this kill the project?"
    );
    // Trailing markup after </p> is not a second block.
    assert_eq!(
        derive_title("<p>Could this kill the project entirely?</p>\n  "),
        "Could this kill the project entirely?"
    );
}

#[test]
fn derived_titles_respect_the_display_cap() {
    let long = format!("<p>{}</p><p>{}</p>", "word ".repeat(60), "tail ".repeat(60));
    let t = derive_title(&long);
    assert!(t.chars().count() <= 120, "{} chars", t.chars().count());
    assert_eq!(t.matches('…').count(), 1, "{t}");
}

#[test]
fn tag_boundaries_are_word_boundaries() {
    // Live-corpus regression (2026-07-17): anchor/br removal welded runs
    // together — "editionshttps://mort.coffee/…", "clienthttps://github…".
    assert_eq!(
        derive_title(
            "<p>SQLite should have (Rust-style) editions<br><a href=\"https://mort.coffee/x\">https://mort.coffee/home/sqlite-editions/</a></p>"
        ),
        "SQLite should have (Rust-style) editions"
    );
}

#[test]
fn drops_url_tokens_from_title() {
    // The link already lives in item.url — inside a title it is noise.
    // "RE:" is an orphaned label once its URL goes, so it goes too.
    assert_eq!(
        derive_title("<p>RE: <a href=\"x\">https://mastodon.social/@arstechnica/1169</a>It feels great to see this</p>"),
        "It feels great to see this"
    );
    // A URL-only toot derives an empty title (caller drops the item).
    assert_eq!(
        derive_title("<p><a href=\"x\">https://example.com/post</a></p>"),
        ""
    );
}

#[test]
fn drops_schemeless_urls_severed_tails_and_orphaned_labels() {
    // Live-corpus residue class (2026-07-17): Mastodon splits a long URL
    // across invisible/ellipsis spans — scheme token, visible scheme-less
    // part, and severed tail all become separate words. All must go, and
    // the "Comments:" label that hung off the URL goes with it.
    assert_eq!(
        derive_title(
            "<p>SQLite should have (Rust-style) editions <a>mort.coffee/home/sqlite-editio</a><span>ns/</span> Comments: <a><span>https://</span>news.ycombinator.com/item?id=4</a><span>8895199</span></p>"
        ),
        "SQLite should have (Rust-style) editions"
    );
    assert_eq!(
        derive_title("<p>Parsing SGF files for fun <a>video.infosec.exchange/w/8iK3NByz1pVVba4kGmycDs</a></p>"),
        "Parsing SGF files for fun"
    );
    // Dev vocabulary is NOT a URL: no slash after the dot-TLD…
    assert_eq!(
        derive_title("<p>why node.js beats socket.io here</p>"),
        "why node.js beats socket.io here"
    );
    // …and no dot-TLD host before the slash.
    assert_eq!(
        derive_title("<p>TCP/IP stack rewrite, 1.80/1.81 diff</p>"),
        "TCP/IP stack rewrite, 1.80/1.81 diff"
    );
    // A plain word after a dropped URL is grammar, not a tail.
    assert_eq!(
        derive_title("<p>see <a>https://example.com/docs</a> rocks anyway</p>"),
        "see rocks anyway"
    );
}

#[test]
fn invisible_span_content_never_reaches_the_title() {
    // Exact live-corpus HTML (2026-07-27, pynews.com.br/@villares): the
    // invisible tail of "sketch-a-day" is purely alphabetic ("ay"), so the
    // is_url_tail digit/path heuristic can NOT catch it — it must die at
    // the span level. Old output: "The sketch-a-day archives and tip jar
    // are ay Code for".
    let live = "<p>The sketch-a-day archives and tip jar are at: <a href=\"https://abav.lugaralgum.com/sketch-a-day\" rel=\"nofollow noopener\" translate=\"no\" target=\"_blank\"><span class=\"invisible\">https://</span><span class=\"ellipsis\">abav.lugaralgum.com/sketch-a-d</span><span class=\"invisible\">ay</span></a> Code for this: <a href=\"https://github.com/villares/sketch-a-day/tree/main/2026/sketch_2026_06_10\" rel=\"nofollow noopener\" translate=\"no\" target=\"_blank\"><span class=\"invisible\">https://</span><span class=\"ellipsis\">github.com/villares/sketch-a-d</span><span class=\"invisible\">ay/tree/main/2026/sketch_2026_06_10</span></a> <a href=\"https://pynews.com.br/tags/Processing\" class=\"mention hashtag\" rel=\"nofollow noopener\" target=\"_blank\">#<span>Processing</span></a> <a href=\"https://pynews.com.br/tags/py5\" class=\"mention hashtag\" rel=\"nofollow noopener\" target=\"_blank\">#<span>py5</span></a></p>";
    let title = derive_title(live);
    assert!(
        !title.contains(" ay ") && !title.ends_with(" ay"),
        "invisible URL tail leaked into title: {title}"
    );
    assert!(
        title.starts_with("The sketch-a-day archives and tip jar"),
        "prose head lost: {title}"
    );
    assert!(!title.contains("https"), "scheme leaked: {title}");

    // The invisible flag must reset at the span close — prose after a
    // link stays visible.
    assert_eq!(
        derive_title(
            "<p>Ship it <a><span class=\"invisible\">https://</span><span class=\"ellipsis\">example.com/x</span><span class=\"invisible\">yz</span></a> today</p>"
        ),
        "Ship it today"
    );
}

#[test]
fn hashtags_trailing_run_dropped_inline_kept() {
    // Trailing run of 2+ = poster metadata — dropped.
    assert_eq!(
        derive_title("<p>Here's how I host my own AIM server <a>#SelfHosting</a> <a>#Networking</a> <a>#Retro</a></p>"),
        "Here's how I host my own AIM server"
    );
    // Inline hashtags are grammar — keep the word, strip the '#'. A SINGLE
    // trailing hashtag is treated as grammar too ("released as #opensource").
    assert_eq!(
        derive_title("<p><a>#Microsoft</a> releases its weird '90s <a>#IRC</a> client as <a>#opensource</a></p>"),
        "Microsoft releases its weird '90s IRC client as opensource"
    );
    // "C#" is not a hashtag.
    assert_eq!(derive_title("<p>why C# wins here</p>"), "why C# wins here");
    // Mastodon's real hashtag markup splits '#' and the word into separate
    // text nodes: `#<span>tag</span>` — they must re-join before the rules.
    assert_eq!(
        derive_title("<p>Learn Linux with David <a>#<span>Linux</span></a> <a>#<span>Ubuntu</span></a> <a>#<span>KaliLinux</span></a></p>"),
        "Learn Linux with David"
    );
    // Mentions split the same way and must re-join (and stay in the title).
    // Surrounding punctuation stays space-separated (tag boundaries are
    // word boundaries) — "( @amnesty )" is readable; "( @ amnesty )" wasn't.
    assert_eq!(
        derive_title("<p>Amnesty (<a>@<span>amnesty</span></a>) released a breakdown</p>"),
        "Amnesty ( @amnesty ) released a breakdown"
    );
}

#[test]
fn hoists_security_id_to_title_front() {
    // A rambling toot that buries the CVE — the id must lead so the headline
    // is useful and the downstream classifier (which reads the title) can tag
    // it as a security advisory.
    let html = "<p>Saturday, but self hosting, so here we go. Earlier this month the HTTP/2 Bomb CVE-2026-49975 dropped, worth a look.</p>";
    let title = derive_title(html);
    assert!(
        title.starts_with("CVE-2026-49975"),
        "CVE should lead the title, got: {title}"
    );

    // GHSA ids too.
    let g = derive_title("<p>heads up, GHSA-w24r-5266-9c3c affects clerk, patch soon</p>");
    assert!(g.starts_with("GHSA-w24r-5266-9c3c"), "got: {g}");

    // Already at the front — don't double-prefix.
    assert_eq!(
        derive_title("<p>CVE-2026-1111 is a nasty one</p>"),
        "CVE-2026-1111 is a nasty one"
    );

    // No id — unchanged behavior.
    assert_eq!(
        derive_title("<p>just a normal post about rust</p>"),
        "just a normal post about rust"
    );
}

#[test]
fn parses_mastodon_api_json() {
    let json = r#"[
        {
            "uri": "https://hachyderm.io/users/x/statuses/1",
            "url": "https://hachyderm.io/@x/1",
            "content": "<p>A neat Rust crate just dropped</p>",
            "account": { "acct": "x@hachyderm.io" },
            "favourites_count": 12, "reblogs_count": 3, "replies_count": 2,
            "tags": [{ "name": "rust" }]
        },
        { "uri": "https://hachyderm.io/users/y/statuses/2", "content": "<p>boost</p>", "reblog": { "id": "9" } }
    ]"#;
    let statuses: Vec<MastodonStatus> = serde_json::from_str(json).unwrap();
    let items: Vec<_> = statuses.into_iter().filter_map(status_to_item).collect();
    assert_eq!(items.len(), 1, "the pure boost is skipped");
    assert_eq!(
        items[0].source_id,
        "https://hachyderm.io/users/x/statuses/1"
    );
    assert_eq!(items[0].title, "A neat Rust crate just dropped");
    let md = items[0].metadata.as_ref().unwrap();
    assert_eq!(md.get("score").and_then(|v| v.as_i64()), Some(15)); // favs + reblogs
}

#[test]
fn parses_mastodon_rss() {
    let xml = r#"<rss><channel>
        <item><title>A tagged post</title><link>https://hachyderm.io/@x/1</link>
          <description>&lt;p&gt;body&lt;/p&gt;</description><guid>https://hachyderm.io/@x/1</guid></item>
    </channel></rss>"#;
    let items = parse_mastodon_rss(xml, "rust", 40);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "A tagged post");
    assert_eq!(items[0].source_id, "https://hachyderm.io/@x/1");
}

#[test]
fn mastodon_source_defaults() {
    let s = MastodonSource::new();
    assert_eq!(s.source_type(), "mastodon");
    assert_eq!(s.name(), "Mastodon");
    assert!(s.config().enabled);
    assert_eq!(s.strategies().len(), 2, "api + rss");
    assert!(s.tags.contains(&"rust"));
}

/// LIVE: federated open-protocol dev-tag fetch produces items credential-free.
/// Run: `cargo test --lib sources::mastodon::tests::live -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "network: verifies live Mastodon dev-tag fetch"]
async fn live_mastodon_produces_items() {
    match MastodonSource::new().fetch_items().await {
        Ok(items) => {
            assert!(!items.is_empty(), "expected dev-tag items from Mastodon");
            let via = items
                .iter()
                .filter_map(|i| i.metadata.as_ref()?.get("via")?.as_str())
                .next()
                .unwrap_or("unknown");
            println!("LIVE mastodon: {} items via {via}", items.len());
        }
        Err(e) => {
            assert!(
                matches!(e, SourceError::Forbidden(_) | SourceError::RateLimited(_)),
                "credential-free paths must fail with an ACTIONABLE error, got {e:?}"
            );
            println!("LIVE mastodon: walled -> surfaced {e:?}");
        }
    }
}
