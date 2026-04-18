// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Entity extraction at ingestion time.
//!
//! Extracts structured entities (CVE IDs) from source item titles and content
//! so they can be stored alongside the item for dedup and grouping.
//! Uses pure string parsing — no regex dependency required.

/// Extract all unique CVE IDs from title and content.
///
/// Returns a JSON array string like `["CVE-2025-1234","CVE-2025-5678"]`,
/// or `None` if no CVE IDs are found.
///
/// Matches the pattern `CVE-YYYY-NNNN+` (4+ digit suffix).
pub fn extract_cve_ids(title: &str, content: &str) -> Option<String> {
    let mut ids: Vec<String> = Vec::new();

    extract_cve_ids_from(title, &mut ids);
    // Only scan first 2000 chars of content (performance guard)
    let content_prefix: String = content.chars().take(2000).collect();
    extract_cve_ids_from(&content_prefix, &mut ids);

    if ids.is_empty() {
        None
    } else {
        // Deduplicate
        ids.sort();
        ids.dedup();
        serde_json::to_string(&ids).ok()
    }
}

/// Extract the first security advisory ID from a string, or `None`.
///
/// Matches both CVE IDs (`CVE-YYYY-NNNNN`) and GitHub Security Advisories
/// (`GHSA-xxxx-xxxx-xxxx`). Used in preemption dedup to group alerts.
pub fn extract_first_advisory_id(text: &str) -> Option<String> {
    // Try CVE first (more common in titles)
    if let Some(cve) = extract_first_cve_id_inner(text) {
        return Some(cve);
    }
    // Then try GHSA
    extract_first_ghsa_id(text)
}

/// Extract the first CVE ID from text.
fn extract_first_cve_id_inner(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 8 < len {
        if (bytes[i] == b'C' || bytes[i] == b'c')
            && (bytes[i + 1] == b'V' || bytes[i + 1] == b'v')
            && (bytes[i + 2] == b'E' || bytes[i + 2] == b'e')
            && bytes[i + 3] == b'-'
        {
            if bytes[i + 4].is_ascii_digit()
                && bytes[i + 5].is_ascii_digit()
                && bytes[i + 6].is_ascii_digit()
                && bytes[i + 7].is_ascii_digit()
            {
                if i + 8 < len && bytes[i + 8] == b'-' {
                    let suffix_start = i + 9;
                    let mut suffix_end = suffix_start;
                    while suffix_end < len && bytes[suffix_end].is_ascii_digit() {
                        suffix_end += 1;
                    }
                    if suffix_end - suffix_start >= 4 {
                        let cve = format!(
                            "CVE-{}-{}",
                            &text[i + 4..i + 8],
                            &text[suffix_start..suffix_end]
                        );
                        return Some(cve);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// Extract the first GHSA ID from text.
/// Format: GHSA-xxxx-xxxx-xxxx where x is lowercase alphanumeric.
fn extract_first_ghsa_id(text: &str) -> Option<String> {
    let text_lower = text.to_lowercase();
    let bytes = text_lower.as_bytes();
    let len = bytes.len();
    // GHSA-xxxx-xxxx-xxxx = 19 chars minimum
    if len < 19 {
        return None;
    }
    let mut i = 0;
    while i + 19 <= len {
        if bytes[i] == b'g'
            && bytes[i + 1] == b'h'
            && bytes[i + 2] == b's'
            && bytes[i + 3] == b'a'
            && bytes[i + 4] == b'-'
        {
            // Validate format: 4 alnum, dash, 4 alnum, dash, 4 alnum
            let seg1 = &bytes[i + 5..i + 9];
            let seg2 = &bytes[i + 10..i + 14];
            let seg3 = &bytes[i + 15..i + 19];
            if bytes[i + 9] == b'-'
                && bytes[i + 14] == b'-'
                && seg1.iter().all(|b| b.is_ascii_alphanumeric())
                && seg2.iter().all(|b| b.is_ascii_alphanumeric())
                && seg3.iter().all(|b| b.is_ascii_alphanumeric())
            {
                // Normalize to uppercase GHSA
                return Some(format!(
                    "GHSA-{}-{}-{}",
                    std::str::from_utf8(seg1).unwrap_or(""),
                    std::str::from_utf8(seg2).unwrap_or(""),
                    std::str::from_utf8(seg3).unwrap_or("")
                ));
            }
        }
        i += 1;
    }
    None
}

/// Scan text for all CVE IDs and append them to the output vec.
fn extract_cve_ids_from(text: &str, out: &mut Vec<String>) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 8 < len {
        if (bytes[i] == b'C' || bytes[i] == b'c')
            && (bytes[i + 1] == b'V' || bytes[i + 1] == b'v')
            && (bytes[i + 2] == b'E' || bytes[i + 2] == b'e')
            && bytes[i + 3] == b'-'
        {
            if bytes[i + 4].is_ascii_digit()
                && bytes[i + 5].is_ascii_digit()
                && bytes[i + 6].is_ascii_digit()
                && bytes[i + 7].is_ascii_digit()
            {
                if i + 8 < len && bytes[i + 8] == b'-' {
                    let suffix_start = i + 9;
                    let mut suffix_end = suffix_start;
                    while suffix_end < len && bytes[suffix_end].is_ascii_digit() {
                        suffix_end += 1;
                    }
                    if suffix_end - suffix_start >= 4 {
                        let cve = format!(
                            "CVE-{}-{}",
                            &text[i + 4..i + 8],
                            &text[suffix_start..suffix_end]
                        );
                        out.push(cve);
                        i = suffix_end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Classify content type at ingestion using the content_dna module.
///
/// Returns the content type slug string (e.g. "security_advisory", "release_notes")
/// for storage in the `content_type` column. Returns `None` only for
/// the default "discussion" type to avoid wasting storage.
pub fn classify_for_storage(title: &str, content: &str, source_type: &str) -> Option<String> {
    let (ct, _) = crate::content_dna::classify_content_for_source(title, content, source_type);
    let slug = ct.slug();
    // Only store non-default types to save space
    if slug == "discussion" {
        None
    } else {
        Some(slug.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_cve() {
        let result = extract_cve_ids("CVE-2025-1234 in OpenSSL", "");
        assert_eq!(result, Some(r#"["CVE-2025-1234"]"#.to_string()));
    }

    #[test]
    fn test_extract_multiple_cves() {
        let result = extract_cve_ids(
            "CVE-2025-1234 and CVE-2025-5678",
            "also mentions cve-2026-9999",
        );
        let parsed: Vec<String> = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.len(), 3);
        assert!(parsed.contains(&"CVE-2025-1234".to_string()));
        assert!(parsed.contains(&"CVE-2025-5678".to_string()));
        assert!(parsed.contains(&"CVE-2026-9999".to_string()));
    }

    #[test]
    fn test_no_cves() {
        assert_eq!(extract_cve_ids("Regular article about Rust", ""), None);
    }

    #[test]
    fn test_dedup_cves() {
        let result = extract_cve_ids(
            "CVE-2025-1234: critical",
            "Details about CVE-2025-1234 vulnerability",
        );
        let parsed: Vec<String> = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn test_extract_first_advisory_id_cve() {
        assert_eq!(
            extract_first_advisory_id("Security: CVE-2025-4321 affects React"),
            Some("CVE-2025-4321".to_string())
        );
        assert_eq!(extract_first_advisory_id("No CVE here"), None);
    }

    #[test]
    fn test_extract_first_advisory_id_ghsa() {
        assert_eq!(
            extract_first_advisory_id("[GHSA-2j53-2c28-gbv2] OpenClaw: Nostr inbound DMs"),
            Some("GHSA-2j53-2c28-gbv2".to_string())
        );
    }

    #[test]
    fn test_extract_ghsa_case_insensitive() {
        assert_eq!(
            extract_first_advisory_id("ghsa-abcd-ef12-3456 vulnerability"),
            Some("GHSA-abcd-ef12-3456".to_string())
        );
    }

    #[test]
    fn test_extract_cve_preferred_over_ghsa() {
        // When both exist, CVE should be returned (it's checked first)
        assert_eq!(
            extract_first_advisory_id("CVE-2025-9999 aka GHSA-abcd-1234-5678"),
            Some("CVE-2025-9999".to_string())
        );
    }

    #[test]
    fn test_cve_case_insensitive() {
        let result = extract_cve_ids("cve-2025-1234 found", "");
        assert_eq!(result, Some(r#"["CVE-2025-1234"]"#.to_string()));
    }

    #[test]
    fn test_short_suffix_rejected() {
        // CVE ID must have 4+ digit suffix
        assert_eq!(extract_cve_ids("CVE-2025-123 is short", ""), None);
    }
}
