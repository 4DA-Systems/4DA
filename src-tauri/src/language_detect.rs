// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Language detection for content items.
//!
//! Uses `whichlang` for fast, zero-dependency detection of 16 major languages.
//! Returns BCP-47 language codes. Falls back to "en" for unrecognized text
//! (safe default -- most developer content is English).

use whichlang::detect_language as wl_detect;

/// Detect the language of a text string. Returns a BCP-47 language code.
///
/// Supports: ar, de, en, es, fr, hi, it, ja, ko, nl, pt, ru, sv, tr, vi, zh
/// Returns "en" for text that doesn't match any supported language or is too
/// short to detect reliably (< 10 non-whitespace characters).
pub fn detect_language(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() < 10 {
        return "en".to_string(); // Too short to detect reliably
    }

    // ASCII-dominant text in dev context is overwhelmingly English.
    // whichlang struggles with short technical English titles containing
    // loanwords (e.g. "Homelab Diagram" → German, "Floci vs Ministack" → Swedish).
    // Require non-ASCII chars for non-English classification of short text.
    if trimmed.len() < 40 && trimmed.is_ascii() {
        return "en".to_string();
    }

    let lang = wl_detect(trimmed);
    whichlang_to_bcp47(lang)
}

/// Returns true if the text appears to be in the given target language.
#[allow(dead_code)]
pub fn is_target_language(text: &str, target_lang: &str) -> bool {
    detect_language(text) == target_lang
}

/// Map whichlang's Lang enum to BCP-47 codes.
fn whichlang_to_bcp47(lang: whichlang::Lang) -> String {
    match lang {
        whichlang::Lang::Ara => "ar",
        whichlang::Lang::Cmn => "zh",
        whichlang::Lang::Deu => "de",
        whichlang::Lang::Eng => "en",
        whichlang::Lang::Fra => "fr",
        whichlang::Lang::Hin => "hi",
        whichlang::Lang::Ita => "it",
        whichlang::Lang::Jpn => "ja",
        whichlang::Lang::Kor => "ko",
        whichlang::Lang::Nld => "nl",
        whichlang::Lang::Por => "pt",
        whichlang::Lang::Rus => "ru",
        whichlang::Lang::Spa => "es",
        whichlang::Lang::Swe => "sv",
        whichlang::Lang::Tur => "tr",
        whichlang::Lang::Vie => "vi",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english() {
        assert_eq!(detect_language("React hooks tutorial for beginners"), "en");
    }

    #[test]
    fn test_detect_japanese() {
        assert_eq!(
            detect_language("\u{30ea}\u{30a2}\u{30af}\u{30c8}\u{30d5}\u{30c3}\u{30af}\u{306e}\u{30c1}\u{30e5}\u{30fc}\u{30c8}\u{30ea}\u{30a2}\u{30eb}\u{521d}\u{5fc3}\u{8005}\u{5411}\u{3051}"),
            "ja"
        );
    }

    #[test]
    fn test_detect_spanish() {
        assert_eq!(
            detect_language(
                "Este es un tutorial completo sobre programaci\u{00f3}n para principiantes"
            ),
            "es"
        );
    }

    #[test]
    fn test_short_text_defaults_to_english() {
        assert_eq!(detect_language("React"), "en");
        assert_eq!(detect_language(""), "en");
    }

    #[test]
    fn test_short_ascii_titles_default_to_english() {
        // whichlang misclassifies these short ASCII titles as non-English
        // but they're clearly English developer content
        assert_eq!(detect_language("Homelab Diagram"), "en");
        assert_eq!(detect_language("question about aws account"), "en");
        assert_eq!(detect_language("Floci vs Ministack vs LocalStack"), "en");
        assert_eq!(
            detect_language("simd-bp128 integer compression library"),
            "en"
        );
    }

    #[test]
    fn test_is_target_language() {
        assert!(is_target_language(
            "This is an English sentence about programming",
            "en"
        ));
        assert!(!is_target_language(
            "This is an English sentence about programming",
            "ja"
        ));
    }
}

/// Detect language using title with content fallback.
/// If title is too short for reliable detection, tries content instead.
pub fn detect_language_with_content(title: &str, content: &str) -> String {
    let title_trimmed = title.trim();
    if title_trimmed.len() >= 20 {
        return detect_language(title_trimmed);
    }

    // Title too short — try content (take first 200 chars for speed)
    let content_trimmed = content.trim();
    if content_trimmed.len() >= 20 {
        let sample: String = content_trimmed.chars().take(200).collect();
        return detect_language(&sample);
    }

    // Both too short — try title anyway if it has at least 10 chars
    if title_trimmed.len() >= 10 {
        return detect_language(title_trimmed);
    }

    "en".to_string()
}

#[cfg(test)]
mod content_fallback_tests {
    use super::*;

    #[test]
    fn test_detect_with_content_fallback() {
        assert_eq!(
            detect_language_with_content(
                "v2.0",
                "Este es un tutorial completo sobre programaci\u{00f3}n para principiantes en espa\u{00f1}ol"
            ),
            "es"
        );
    }

    #[test]
    fn test_detect_with_content_long_title() {
        assert_eq!(
            detect_language_with_content(
                "React hooks tutorial for beginners in web development",
                "Este es contenido en espa\u{00f1}ol"
            ),
            "en"
        );
    }
}
