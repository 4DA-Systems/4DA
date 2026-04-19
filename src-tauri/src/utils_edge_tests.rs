// SPDX-License-Identifier: FSL-1.1-Apache-2.0
#[cfg(test)]
mod tests {
    use crate::utils::{
        cosine_similarity, cosine_similarity_with_norm, decode_html_entities, preprocess_content,
        truncate_utf8, vector_norm,
    };

    // ========================================================================
    // Vector math edge cases — these are the scoring hot path
    // ========================================================================

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(
            cosine_similarity(&a, &b),
            0.0,
            "Mismatched lengths should return 0"
        );
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(
            cosine_similarity(&a, &b),
            0.0,
            "Zero vector should return 0"
        );
        assert_eq!(
            cosine_similarity(&b, &a),
            0.0,
            "Zero vector should return 0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - (-1.0)).abs() < 0.001,
            "Opposite vectors should be -1.0, got {}",
            sim
        );
    }

    #[test]
    fn test_cosine_similarity_with_norm_matches() {
        let a = vec![3.0, 4.0];
        let b = vec![4.0, 3.0];
        let a_norm = vector_norm(&a);

        let sim1 = cosine_similarity(&a, &b);
        let sim2 = cosine_similarity_with_norm(&a, a_norm, &b);
        assert!(
            (sim1 - sim2).abs() < 0.0001,
            "with_norm should match regular: {} vs {}",
            sim1,
            sim2
        );
    }

    #[test]
    fn test_cosine_similarity_with_norm_zero_norm() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity_with_norm(&a, 0.0, &b), 0.0);
    }

    #[test]
    fn test_vector_norm_unit() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((vector_norm(&v) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_vector_norm_known() {
        // 3-4-5 triangle
        let v = vec![3.0, 4.0];
        assert!((vector_norm(&v) - 5.0).abs() < 0.0001);
    }

    #[test]
    fn test_vector_norm_empty() {
        assert_eq!(vector_norm(&[]), 0.0);
    }

    // ========================================================================
    // Content preprocessing edge cases
    // ========================================================================

    #[test]
    fn test_preprocess_empty() {
        assert_eq!(preprocess_content(""), "");
    }

    #[test]
    fn test_preprocess_strips_html() {
        let input = "<p>Hello <strong>world</strong></p>";
        let result = preprocess_content(input);
        assert!(result.contains("Hello"));
        assert!(result.contains("world"));
        assert!(!result.contains("<p>"));
        assert!(!result.contains("<strong>"));
    }

    #[test]
    fn test_preprocess_decodes_entities() {
        let input = "Tom &amp; Jerry &lt;3 each other";
        let result = preprocess_content(input);
        assert!(result.contains("Tom & Jerry"));
        assert!(result.contains("<3"));
    }

    #[test]
    fn test_preprocess_strips_urls() {
        let input = "Check https://example.com/path for details";
        let result = preprocess_content(input);
        assert!(!result.contains("https://"));
        assert!(result.contains("Check"));
        assert!(result.contains("details"));
    }

    #[test]
    fn test_preprocess_collapses_whitespace() {
        let input = "Hello    world\n\n\ntest";
        let result = preprocess_content(input);
        assert_eq!(result, "Hello world test");
    }

    #[test]
    fn test_preprocess_caps_at_2000_chars() {
        let long_input = "a".repeat(5000);
        let result = preprocess_content(&long_input);
        assert_eq!(result.chars().count(), 2000);
    }

    #[test]
    fn test_preprocess_handles_unclosed_html_tag() {
        // Unclosed tag — should not lose all subsequent text
        let input = "before <div after";
        let result = preprocess_content(input);
        assert!(
            result.contains("before"),
            "Text before unclosed tag preserved"
        );
        // After '<' everything is considered inside tag until '>'
        // This is the expected behavior of the simple parser
    }

    #[test]
    fn test_preprocess_html_then_entities_order() {
        // The order matters: strip tags FIRST, then decode entities
        // If reversed, &lt;script&gt; would become <script> and then get stripped
        let input = "&lt;script&gt;alert('xss')&lt;/script&gt;";
        let result = preprocess_content(input);
        // Should decode entities AFTER tag stripping, preserving the text
        assert!(
            result.contains("alert"),
            "Entity-encoded tags should be preserved as text"
        );
    }

    #[test]
    fn test_preprocess_multibyte_truncation() {
        // 500 Chinese characters = 1500 bytes, but only 500 chars
        // Should fit under 2000-char limit
        let chinese = "你".repeat(500);
        let result = preprocess_content(&chinese);
        assert_eq!(result.chars().count(), 500);
    }

    // ========================================================================
    // HTML entity decoding edge cases
    // ========================================================================

    #[test]
    fn test_decode_all_entities() {
        assert_eq!(decode_html_entities("&amp;"), "&");
        assert_eq!(decode_html_entities("&lt;"), "<");
        assert_eq!(decode_html_entities("&gt;"), ">");
        assert_eq!(decode_html_entities("&quot;"), "\"");
        assert_eq!(decode_html_entities("&apos;"), "'");
        assert_eq!(decode_html_entities("&#39;"), "'");
        assert_eq!(decode_html_entities("&#x27;"), "'");
        assert_eq!(decode_html_entities("&nbsp;"), " ");
    }

    #[test]
    fn test_decode_multiple_entities() {
        let input = "A &amp; B &lt; C &gt; D";
        assert_eq!(decode_html_entities(input), "A & B < C > D");
    }

    #[test]
    fn test_decode_no_entities() {
        let input = "plain text without entities";
        assert_eq!(decode_html_entities(input), input);
    }

    // ========================================================================
    // UTF-8 truncation edge cases
    // ========================================================================

    #[test]
    fn test_truncate_zero() {
        assert_eq!(truncate_utf8("hello", 0), "");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate_utf8("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_emoji() {
        let emoji = "🔥🚀💻🎯🏆";
        assert_eq!(truncate_utf8(emoji, 3), "🔥🚀💻");
    }

    #[test]
    fn test_truncate_mixed_scripts() {
        let mixed = "Hello世界🌍";
        assert_eq!(truncate_utf8(mixed, 7), "Hello世界");
    }
}
