// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

/// Detect language using the title, falling back to the body when the title is
/// too short or ambiguous to classify on its own.
///
/// Closes a precision gap in title-only detection: a short ASCII title can be
/// Romance-language text WITHOUT accents (e.g. Spanish "Busco un Selector de
/// Colores"), which `detect_language` deliberately defaults to "en" because the
/// same short-ASCII heuristic protects short English dev titles ("Homelab
/// Diagram") from being misread as foreign. When the title lands in that
/// ambiguous zone, we consult the body: a confidently non-English content sample
/// reveals the real language, while genuinely English items (English title +
/// English body) stay "en" — so this never suppresses English content.
pub fn detect_language_with_content(title: &str, content: &str) -> String {
    let title_trimmed = title.trim();
    let title_lang = detect_language(title_trimmed);

    // If detect_language classified the title CONFIDENTLY — it found a real
    // language, OR the title was long enough / non-ASCII enough that the
    // short-ASCII "default to English" guard never fired — trust the title.
    let title_forced_english =
        title_lang == "en" && title_trimmed.len() < 40 && title_trimmed.is_ascii();
    if !title_forced_english {
        return title_lang;
    }

    // Ambiguous short-ASCII title defaulted to English. Consult the body.
    let content_trimmed = content.trim();
    if content_trimmed.len() >= 40 {
        let sample: String = content_trimmed.chars().take(400).collect();
        let content_lang = detect_language(&sample);
        if content_lang != "en" {
            return content_lang;
        }
    }

    "en".to_string()
}

/// True if `c` is a letter belonging to a non-Latin script that a Latin-script
/// (e.g. English) reader cannot read: CJK, kana, Hangul, Cyrillic, Hebrew,
/// Arabic, Devanagari, Thai.
fn is_non_latin_letter(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF |   // CJK (incl. ext-A)
        0x3040..=0x30FF |                      // Hiragana + Katakana
        0x1100..=0x11FF | 0xAC00..=0xD7AF |    // Hangul Jamo + syllables
        0x0400..=0x04FF |                      // Cyrillic
        0x0590..=0x05FF |                      // Hebrew
        0x0600..=0x06FF |                      // Arabic
        0x0900..=0x097F |                      // Devanagari
        0x0E00..=0x0E7F                        // Thai
    )
}

/// Fraction (0.0–1.0) of a string's letters that belong to a non-Latin script.
/// Digits, punctuation, emoji, and whitespace are ignored. Returns 0.0 for text
/// with no letters.
pub fn non_latin_script_ratio(text: &str) -> f32 {
    let mut letters: u32 = 0;
    let mut non_latin: u32 = 0;
    for c in text.chars() {
        if is_non_latin_letter(c) {
            letters += 1;
            non_latin += 1;
        } else if c.is_alphabetic() {
            letters += 1;
        }
    }
    if letters == 0 {
        return 0.0;
    }
    non_latin as f32 / letters as f32
}

/// Whether `text` is predominantly a non-Latin script (≥ 35% of its letters).
///
/// Used to filter foreign-language items at ingestion that an English reader
/// cannot read — catching items even when `detect_language` misclassifies a
/// mostly-foreign title (e.g. a CJK title with a couple of ASCII tokens). A
/// mostly-English title containing a few foreign characters stays below the
/// threshold and is retained.
pub fn is_predominantly_non_latin(text: &str) -> bool {
    non_latin_script_ratio(text) >= 0.35
}

#[cfg(test)]
mod script_ratio_tests {
    use super::*;

    #[test]
    fn pure_english_is_not_foreign() {
        assert!(!is_predominantly_non_latin(
            "React hooks tutorial for beginners"
        ));
        assert!(!is_predominantly_non_latin(
            "simd-bp128 integer compression library"
        ));
    }

    #[test]
    fn mostly_english_with_a_few_cjk_is_kept() {
        // "Mina the Hollower 攻略 Wiki" — 2 CJK chars among ~19 Latin letters.
        assert!(!is_predominantly_non_latin(
            "Mina the Hollower \u{653b}\u{7565} Wiki"
        ));
    }

    #[test]
    fn predominantly_chinese_is_foreign() {
        // Even with ASCII tokens (AI, agent, context) the title is mostly CJK.
        assert!(is_predominantly_non_latin(
            "\u{4f60}\u{7684} AI agent \u{4e0d}\u{7b28}\u{ff0c}\u{662f}\u{4f60}\u{9905}\u{7684} context \u{4e0d}\u{884c}"
        ));
    }

    #[test]
    fn cyrillic_and_arabic_are_foreign() {
        assert!(is_predominantly_non_latin(
            "\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442} \u{043c}\u{0438}\u{0440}"
        ));
        assert!(is_predominantly_non_latin(
            "\u{0645}\u{0631}\u{062d}\u{0628}\u{0627} \u{0628}\u{0627}\u{0644}\u{0639}\u{0627}\u{0644}\u{0645}"
        ));
    }

    #[test]
    fn no_letters_is_not_foreign() {
        assert!(!is_predominantly_non_latin("v2.0.1 — 1234 !!!"));
    }
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

    #[test]
    fn test_short_ascii_foreign_title_resolved_via_content() {
        // The precision gap: a short ASCII title (no accents) that the title-only
        // heuristic defaults to "en", but whose body is clearly Spanish. The body
        // reveals the real language so the language gate can cap it.
        assert_eq!(
            detect_language_with_content(
                "Busco un Selector de Colores",
                "Estoy desarrollando una aplicaci\u{00f3}n en Kotlin y necesito un selector de \
                 colores. \u{00bf}Alguien conoce una biblioteca recomendada para esto?"
            ),
            "es"
        );
    }

    #[test]
    fn test_short_ascii_english_title_with_english_body_stays_english() {
        // Regression guard: short ASCII English title + English body must NOT be
        // reclassified as foreign — that would suppress legitimate English content.
        assert_eq!(
            detect_language_with_content(
                "Quick question about hooks",
                "I'm using React hooks and wondering about the best pattern for data \
                 fetching in a dashboard component with Suspense."
            ),
            "en"
        );
    }

    #[test]
    fn test_short_ascii_title_with_no_body_keeps_english_default() {
        // No body to consult — the protective English default for short ASCII
        // dev titles still holds.
        assert_eq!(detect_language_with_content("Floci vs Ministack", ""), "en");
    }
}
