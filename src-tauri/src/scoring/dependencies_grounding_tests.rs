// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Grounding-precision regressions from the 2026-10-02 adversarial audit:
//! org-name packages (`openai`), raw-substring context words, and the
//! non-dev corroboration flag. Live titles are quoted verbatim.

use super::*;

fn dep(name: &str, ecosystem: &str, is_dev: bool) -> DepInfo {
    DepInfo {
        package_name: name.to_string(),
        version: Some("4.2.0".to_string()),
        is_dev,
        is_direct: true,
        search_terms: extract_search_terms(name),
        ecosystem: ecosystem.to_string(),
        project_paths: vec!["d:/runyourempire/navcal".to_string()],
        project_relevance: 1.0,
    }
}

fn ctx_with(deps: &[DepInfo]) -> ACEContext {
    let mut ctx = ACEContext::default();
    for d in deps {
        ctx.dependency_info
            .insert(normalize_package_name(&d.package_name), d.clone());
    }
    ctx
}

fn grounded(title: &str, content: &str, ctx: &ACEContext) -> bool {
    let (matches, _) = match_dependencies(title, content, &[], ctx);
    is_strongly_grounded(&matches)
}

fn names(title: &str, content: &str, ctx: &ACEContext, pkg: &str) -> bool {
    let (matches, _) = match_dependencies(title, content, &[], ctx);
    matches
        .iter()
        .any(|m| m.package_name == pkg && m.corroborated)
}

// ── 1. Org-name packages ────────────────────────────────────────────────────

#[test]
fn openai_company_news_does_not_ground_the_openai_package() {
    let ctx = ctx_with(&[dep("openai", "javascript", false)]);
    let cases: &[(&str, &str)] = &[
        (
            "OpenAI still doesn't seem to have a handle on all of its rogue AI activity",
            "On Friday, OpenAI published a new site devoted to misalignment reports. \
             The rogue agent incidents, a security concern, are important: models \
             hacked another firm and tried to exploit systems.",
        ),
        (
            "OpenAI agents tried to bruteforce a UN website's API fields",
            "From 13 April - 19 June 2026, OpenAI agents scanned UNCTAD's API ~16,500 \
             times, using proxies, obfuscation, and Google's XSS game. # security # cyberattack",
        ),
        (
            "OpenAI halts frontier RL training after Astra crosses Critical cyber threshold",
            "",
        ),
        (
            "OpenAI halts testing, slows development after rogue model hacked Hugging Face",
            "OpenAI is slowing down its artificial intelligence development after two \
             OpenAI models broke out of their testing environment and hacked another firm.",
        ),
        (
            "GPT-6 Astra: OpenAI GPT-5.4 successor shows cyber exploit capability",
            "OpenAI said the vulnerability research capability crossed a threshold.",
        ),
        (
            "OpenAI's Rogue AI Agent Hacked More Than Just Hugging Face",
            "",
        ),
        (
            "What Is OpenAI's Decisions API?",
            "Existing apps must migrate; the old endpoint is removed. Developers \
             build with general-purpose OpenAI models.",
        ),
        (
            "Meta's Muse appears to use an OpenAI model labeled muse-special",
            "gpt responses model client via magi native azure openai lane. \
             The muse-special model is possibly an openai model client.",
        ),
        (
            "OpenAI after DevDay 2026: known issues for serious development work",
            "the mcp client has no auto-reconnect (openai/codex#11489, \
             openai-apps-sdk-examples#241, openai community).",
        ),
        (
            "FTC opens probe into AI giants including Anthropic and OpenAI",
            "The investigation covers security and safety practices.",
        ),
    ];
    for (title, content) in cases {
        assert!(
            !names(title, content, &ctx, "openai"),
            "company news must not NAME the openai package: {title}"
        );
        assert!(
            !grounded(title, content, &ctx),
            "company news must not ground the openai package: {title}"
        );
    }
}

#[test]
fn genuine_openai_package_news_still_grounds() {
    let ctx = ctx_with(&[dep("openai", "javascript", false)]);
    let cases: &[(&str, &str)] = &[
        ("openai v7.25.0 released", ""),
        (
            "OpenAI Node SDK v5 breaking changes",
            "The client now requires Node 20.",
        ),
        (
            "openai npm package compromised",
            "A malicious version of the package was published to npm.",
        ),
        ("openai@4.2.0 ships streaming helpers", ""),
        (
            "Migrating the openai client to Responses",
            "Run `pip install openai` and then import openai.",
        ),
        (
            "OpenAI deprecates the Assistants API",
            "Migrate to the Responses API by 2027.",
        ),
        ("openai 4.2.0 fixes a token leak", ""),
    ];
    for (title, content) in cases {
        assert!(
            names(title, content, &ctx, "openai"),
            "package news must name openai: {title}"
        );
        assert!(
            grounded(title, content, &ctx),
            "package news must ground openai: {title}"
        );
    }
    // Body-only code use NAMES the package (corroborated) but a 0.2 content
    // hit alone stays below the strong floor — unchanged behaviour.
    let (title, content) = (
        "Migrating to the new client",
        "Run `pip install openai` and then import openai.",
    );
    assert!(names(title, content, &ctx, "openai"));
    assert!(!grounded(title, content, &ctx));
}

#[test]
fn org_name_guard_is_scoped_to_org_names() {
    // A distinctive tech package keeps the existing rule: security context
    // near the name corroborates an editorial vulnerability story.
    let ctx = ctx_with(&[dep("axios", "javascript", false)]);
    assert!(grounded(
        "Axios compromised in supply-chain attack",
        "Attackers published a malicious version.",
        &ctx
    ));
    assert!(crate::package_ambiguity::is_org_name_package("OpenAI"));
    assert!(crate::package_ambiguity::is_org_name_package("stripe"));
    assert!(!crate::package_ambiguity::is_org_name_package("axios"));
    assert!(!crate::package_ambiguity::is_org_name_package("typescript"));
}

#[test]
fn stripe_company_news_vs_stripe_sdk_release() {
    let ctx = ctx_with(&[dep("stripe", "javascript", false)]);
    assert!(!grounded(
        "Stripe raises at $120B valuation and launches stablecoin accounts",
        "The payments company said security and compliance improved.",
        &ctx
    ));
    assert!(grounded("stripe-node: stripe v18.0.0 released", "", &ctx));
    assert!(grounded(
        "Stripe Node library v18 drops support for Node 16",
        "",
        &ctx
    ));
}

#[test]
fn vercel_platform_news_does_not_ground_the_vercel_cli() {
    // Live: `vercel` is a devDependency (the CLI). "Vercel AI SDK" names a
    // different package (`ai`); platform news is not a CLI release.
    let ctx = ctx_with(&[dep("vercel", "javascript", true)]);
    for title in [
        "Vercel AI SDK Alternatives and Where to Host Them in 2026",
        "Vercel now supports the Bun Runtime",
        "Vercel CDN no longer caches responses with Vary: Cookie",
    ] {
        assert!(
            !names(
                title,
                "compare frameworks; npm install is not needed",
                &ctx,
                "vercel"
            ),
            "{title}"
        );
    }
    assert!(names("vercel 41.2.0 released", "", &ctx, "vercel"));
}

#[test]
fn org_version_literal_must_be_immediate() {
    // GPT-5.4 sits within 20 bytes after "openai" — the lax adjacency rule
    // would read it as openai's version.
    assert!(!has_immediate_version_literal("openai gpt-5.4 launches", 6));
    assert!(!has_immediate_version_literal("openai 2026 roadmap", 6));
    assert!(!has_immediate_version_literal("openai 4o model", 6));
    assert!(has_immediate_version_literal("openai 4.2.0 shipped", 6));
    assert!(has_immediate_version_literal("openai@4.2.0", 6));
    assert!(has_immediate_version_literal("openai v5 shipped", 6));
    assert!(!has_immediate_version_literal("openai", 6));
}

// ── 2. Context words are tokens, not substrings ─────────────────────────────

#[test]
fn language_context_words_do_not_match_inside_other_words() {
    for text in [
        "gemini 3 pro is out",           // gem
        "our ci pipeline got faster",    // pip
        "an important announcement",     // import
        "a liberal reading of the rule", // lib
        "deploy on friday",              // dep
        "the requirement is unclear",    // require
        "bundle sizes shrink",           // bun
    ] {
        assert!(
            !has_language_context_nearby(text, 0, text.len()),
            "no language context in: {text}"
        );
    }
    for text in [
        "install the gem",
        "pip install foo",
        "import foo from bar",
        "the lib is small",
        "a new dep landed",
        "add it with npm",
        "published to crates.io",
        "two packages updated",
        "the dependencies changed",
        "installed it yesterday",
        "the library ships",
    ] {
        assert!(
            has_language_context_nearby(text, 0, text.len()),
            "language context in: {text}"
        );
    }
}

#[test]
fn window_edge_cannot_cut_a_word_into_a_context_token() {
    // "important" truncated at the window end used to leave "import".
    let text = "zzzzzzzzzz foo zz important";
    let pos = text.find("foo").unwrap_or(0);
    let end_of_import = text.find("important").unwrap_or(0) + "import".len();
    let window = end_of_import - pos;
    assert!(!has_language_context_nearby(text, pos, window));
}

#[test]
fn security_markers_need_a_word_start() {
    for text in [
        "the event dispatch loop",  // patch
        "a 10-day trip report",     // 0-day
        "insecurity in the market", // security
    ] {
        assert!(
            !has_security_context_nearby(text, 0, text.len()),
            "no security context in: {text}"
        );
    }
    for text in [
        "patched in 1.2.4",
        "a zero-day in the wild",
        "security advisory published",
        "cve-2026-1234 disclosed",
        "multiple vulnerabilities fixed",
        "the package was compromised",
    ] {
        assert!(
            has_security_context_nearby(text, 0, text.len()),
            "security context in: {text}"
        );
    }
}

#[test]
fn gemini_title_does_not_corroborate_a_word_like_dep() {
    // Ambiguous single-token dep "notify" + "Gemini" in title: "gem" used to
    // count as language context and grant the 0.4 ambiguous-title credit.
    let ctx = ctx_with(&[dep("notify", "rust", false)]);
    let (matches, _) = match_dependencies(
        "Gemini can now notify you about calendar events",
        "",
        &[],
        &ctx,
    );
    assert!(
        matches
            .iter()
            .all(|m| m.confidence < STRONG_GROUNDING_CONFIDENCE),
        "{matches:?}"
    );
}

// ── 3. Corroboration's dependency flag is non-dev ───────────────────────────

#[test]
fn non_dev_grounding_predicate_excludes_dev_deps() {
    let mk = |is_dev: bool| DepMatch {
        package_name: "vitest".to_string(),
        confidence: 0.6,
        version_delta: VersionDelta::Unknown,
        is_dev,
        is_direct: true,
        version: None,
        ecosystem: "javascript".to_string(),
        corroborated: true,
        project_paths: Vec::new(),
        raw_name: None,
    };
    assert!(is_strongly_grounded(&[mk(true)]));
    assert!(!is_strongly_grounded_non_dev(&[mk(true)]));
    assert!(is_strongly_grounded_non_dev(&[mk(false)]));
}
