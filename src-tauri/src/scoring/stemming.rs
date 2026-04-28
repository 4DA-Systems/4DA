// SPDX-License-Identifier: FSL-1.1-Apache-2.0

/// Lightweight English stemmer for scoring keyword matching.
///
/// Uses suffix-stripping rules tuned for developer content (not a full
/// Porter/Snowball stemmer — those over-stem technical terms). Returns
/// the stemmed form for comparison; original text is never modified.

/// Collapse a doubled final consonant: "debugg" → "debug", "shipp" → "ship".
/// Only fires after suffix stripping produces an artificial double.
fn collapse_doubled_final(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 4 {
        let last = bytes[bytes.len() - 1];
        let prev = bytes[bytes.len() - 2];
        if last == prev
            && last.is_ascii_lowercase()
            && !matches!(last, b'a' | b'e' | b'i' | b'o' | b'u')
        {
            return s[..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

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
                let result = format!("{base}{replacement}");
                return collapse_doubled_final(&result);
            }
        }
    }

    // Trailing -s (but not -ss like "class", "process")
    if w.len() >= 4 && w.ends_with('s') && !w.ends_with("ss") {
        return w[..w.len() - 1].to_string();
    }

    w
}

#[cfg(test)]
fn stems_match(a: &str, b: &str) -> bool {
    stems_equiv(&stem(a), &stem(b))
}

/// Compare two already-stemmed values with trailing-e tolerance.
/// Handles: compile ("compile") vs compiler ("compil"), configure vs configurator, etc.
pub(crate) fn stems_equiv(stem_a: &str, stem_b: &str) -> bool {
    if stem_a == stem_b {
        return true;
    }
    let (longer, shorter) = if stem_a.len() > stem_b.len() {
        (stem_a, stem_b)
    } else {
        (stem_b, stem_a)
    };
    longer.len() == shorter.len() + 1 && longer.ends_with('e') && longer.starts_with(shorter)
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
    // -ilation → -ile (compilation → compile)
    ("ilation", "ile"),
    // -ation → -ate (authentication → authenticate, validation → validate)
    // Short-word guard prevents: nation (base "n" < 3), station (base "st" < 3)
    ("ation", "ate"),
    // -ting → -t (testing → test, routing → rout... too aggressive for "routing")
    // Instead be selective:
    ("sting", "st"), // testing → test
    ("lding", "ld"), // building → build
    ("nding", "nd"), // binding → bind
    ("ding", "d"),   // loading → load (careful: "coding" → "cod" if base < 3, guarded)
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

    #[test]
    fn test_doubled_consonant_collapse() {
        assert!(stems_match("debug", "debugging"));
        assert!(stems_match("ship", "shipping"));
        assert!(stems_match("log", "logging"));
        assert!(stems_match("map", "mapping"));
        assert!(stems_match("run", "running"));
        assert!(stems_match("set", "setting"));
    }

    #[test]
    fn test_ation_suffix() {
        assert!(stems_match("authenticate", "authentication"));
        assert!(stems_match("validate", "validation"));
        assert!(stems_match("migrate", "migration"));
    }

    #[test]
    fn test_compilation() {
        assert!(stems_match("compile", "compilation"));
        assert!(stems_match("compile", "compiler"));
    }

    #[test]
    fn test_stems_equiv_trailing_e() {
        assert!(stems_equiv("compile", "compil"));
        assert!(stems_equiv("configure", "configur"));
        assert!(!stems_equiv("rust", "rustic"));
        assert!(!stems_equiv("test", "testing"));
    }
}
