// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/// Lightweight English stemmer for scoring keyword matching.
///
/// Uses suffix-stripping rules tuned for developer content (not a full
/// Porter/Snowball stemmer — those over-stem technical terms). Returns
/// the stemmed form for comparison; original text is never modified.

/// Stem a single word. Returns a lowercase stemmed form.
/// Short words (< 4 chars) are returned as-is to avoid collisions.
pub(crate) fn stem(word: &str) -> String {
    let w = word.to_lowercase();
    if w.len() < 4 {
        return w;
    }

    // Try suffix rules in order of specificity (longest first)
    for &(suffix, replacement) in SUFFIX_RULES {
        if let Some(base) = w.strip_suffix(suffix) {
            if base.len() >= 3 {
                return format!("{base}{replacement}");
            }
        }
    }

    // Trailing -s (but not -ss like "class", "process")
    if w.len() >= 4 && w.ends_with('s') && !w.ends_with("ss") {
        return w[..w.len() - 1].to_string();
    }

    w
}

/// Returns true if two words share the same stem.
pub(crate) fn stems_match(a: &str, b: &str) -> bool {
    let sa = stem(a);
    let sb = stem(b);
    sa == sb
}

/// Suffix rules: (suffix_to_strip, replacement).
/// Order matters — longer/more specific suffixes first.
/// Tuned for developer content: avoids over-stemming technical terms.
const SUFFIX_RULES: &[(&str, &str)] = &[
    // -ization → -ize (optimization → optim + ize... too aggressive)
    // Instead: -ization → -iz
    ("ization", "iz"),
    ("isation", "iz"),
    // -uration → -ure (configuration → configure base)
    ("uration", "ure"),
    // -ational → -ate
    ("ational", "ate"),
    // -fulness → -ful
    ("fulness", "ful"),
    // -iveness → -ive
    ("iveness", "ive"),
    // -lessly → -less
    ("lessly", "less"),
    // -ements → -e
    ("ements", "e"),
    // -nesses → -ness... skip, too rare
    // -ating → -ate
    ("ating", "ate"),
    // -uring → -ure
    ("uring", "ure"),
    // -izing → -ize
    ("izing", "ize"),
    // -ising → -ise
    ("ising", "ise"),
    // -ively → -ive
    ("ively", "ive"),
    // -ement → -e
    ("ement", "e"),
    // -ation → -ate  (but careful: "nation" → "nate" is wrong)
    // Skip this — too many false positives
    // -ting → -t (testing → test, routing → rout... too aggressive for "routing")
    // Instead be selective:
    ("sting", "st"),    // testing → test
    ("lding", "ld"),    // building → build
    ("nding", "nd"),    // binding → bind
    ("ding", "d"),      // loading → load (careful: "coding" → "cod" if base < 3, guarded)
    // -ness → ""
    ("ness", ""),
    // -ment → ""
    ("ment", ""),
    // -lable → -le (scalable → scale)
    ("lable", "le"),
    // -able → -e (when stem likely had trailing e: observable → observe)
    ("vable", "ve"),
    // -able → "" (general case)
    ("able", ""),
    // -ible → ""
    ("ible", ""),
    // -ence → ""
    ("ence", ""),
    // -ance → ""
    ("ance", ""),
    // -ling → -le
    ("ling", "le"),
    // -ying → -y
    ("ying", "y"),
    // -ier → -y
    ("ier", "y"),
    // -ies → -y
    ("ies", "y"),
    // -ive → "" (reactive → react)
    ("ive", ""),
    // -ful → ""
    ("ful", ""),
    // -ous → ""
    ("ous", ""),
    // -ize → "iz"
    ("ize", "iz"),
    // -ise → "iz"
    ("ise", "iz"),
    // -urity → -ure (security → secure, maturity → mature)
    ("urity", "ure"),
    // -ity → ""
    ("ity", ""),
    // -ing → "" (but only with sufficient base)
    ("ing", ""),
    // -ion → ""
    ("ion", ""),
    // -ers → ""
    ("ers", ""),
    // -ors → ""
    ("ors", ""),
    // -als → ""
    ("als", ""),
    // -ed → ""
    ("ed", ""),
    // -ly → ""
    ("ly", ""),
    // -er → ""
    ("er", ""),
    // -or → ""
    ("or", ""),
    // -es → ""
    ("es", ""),
    // -al → ""
    ("al", ""),
    // -s (but not -ss like "class" or "process")
    // Handled specially below
];

/// Extended stem that also handles trailing -s (but not -ss).
pub(crate) fn stem_extended(word: &str) -> String {
    let s = stem(word);
    if s.len() >= 4 && s.ends_with('s') && !s.ends_with("ss") {
        s[..s.len() - 1].to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_stemming() {
        assert!(stems_match("test", "testing"));
        assert!(stems_match("test", "tested"));
        assert!(stems_match("test", "tester"));
        assert!(stems_match("test", "tests"));
    }

    #[test]
    fn test_developer_terms() {
        assert!(stems_match("build", "building"));
        assert!(stems_match("build", "builder"));
        assert!(stems_match("deploy", "deployment"));
        assert!(stems_match("configure", "configuration"));
        assert!(stems_match("optimize", "optimization"));
        assert!(stems_match("perform", "performance"));
        assert!(stems_match("scale", "scalable"));
        assert!(stems_match("observe", "observable"));
        assert!(stems_match("react", "reactive"));
        assert!(stems_match("secure", "security"));
    }

    #[test]
    fn test_short_words_unchanged() {
        assert_eq!(stem("go"), "go");
        assert_eq!(stem("ai"), "ai");
        assert_eq!(stem("api"), "api");
    }

    #[test]
    fn test_no_false_positives() {
        // "rust" should NOT match "rustic" through stemming
        assert!(!stems_match("rust", "rustic"));
        // "go" too short to stem
        assert!(!stems_match("go", "going"));
        // "class" should not lose its -ss
        assert_eq!(stem("class"), "class");
    }

    #[test]
    fn test_plurals() {
        assert!(stems_match("dependency", "dependencies"));
        assert!(stems_match("library", "libraries"));
        assert!(stems_match("query", "queries"));
    }
}
