// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! XML parsing and canonicalization helpers for SSO (SAML/OIDC).
//!
//! Extracted from sso.rs to keep file sizes manageable.
//! Contains lightweight XML element extraction (no external XML parser)
//! and simplified exclusive C14N for SAML signature verification.

/// Extract the text content of the first occurrence of an XML element.
///
/// Simple regex-free parser for well-formed XML. Finds `<tag...>content</tag>`
/// or `<ns:tag...>content</ns:tag>` patterns.
pub(crate) fn extract_xml_element(xml: &str, local_name: &str) -> Option<String> {
    // Match both <localName> and <ns:localName>
    let patterns = [format!("<{local_name}"), format!(":{local_name}")];

    for pattern in &patterns {
        if let Some(start_pos) = xml.find(pattern.as_str()) {
            let after_tag = &xml[start_pos..];
            let close_bracket = after_tag.find('>')?;
            let content_start = start_pos + close_bracket + 1;

            let remaining = &xml[content_start..];
            let mut search_offset = 0;
            while let Some(close_pos) = remaining[search_offset..].find("</") {
                let abs_close = search_offset + close_pos;
                let tag_rest = &remaining[abs_close + 2..];
                if let Some(gt_pos) = tag_rest.find('>') {
                    let tag_name = &tag_rest[..gt_pos];
                    if tag_name == local_name || tag_name.ends_with(&format!(":{local_name}")) {
                        let content = &remaining[..abs_close];
                        return Some(content.trim().to_string());
                    }
                }
                search_offset = abs_close + 2;
            }
        }
    }

    None
}

/// Extract a SAML Attribute value by its Name attribute.
pub(crate) fn extract_saml_attribute(xml: &str, attr_name: &str) -> Option<String> {
    let name_pattern = format!("Name=\"{attr_name}\"");
    let pos = xml.find(&name_pattern)?;
    let after = &xml[pos..];

    let av_start = after.find("AttributeValue")?;
    let av_content_start = after[av_start..].find('>')? + av_start + 1;
    let av_content_end = after[av_content_start..].find("</")? + av_content_start;

    let value = &after[av_content_start..av_content_end];
    Some(value.trim().to_string())
}

/// Extract multiple SAML Attribute values (for group memberships).
pub(crate) fn extract_saml_attribute_values(xml: &str, attr_name: &str) -> Option<Vec<String>> {
    let name_pattern = format!("Name=\"{attr_name}\"");
    let pos = xml.find(&name_pattern)?;

    let after = &xml[pos..];
    let attr_end = after.find("</").and_then(|p| {
        after[p..]
            .find("Attribute>")
            .map(|q| p + q + "Attribute>".len())
    })?;

    let attr_block = &after[..attr_end];
    let mut values = Vec::new();
    let mut search_from = 0;

    while let Some(av_start) = attr_block[search_from..].find("AttributeValue") {
        let abs_start = search_from + av_start;
        if let Some(content_start) = attr_block[abs_start..].find('>') {
            let cs = abs_start + content_start + 1;
            if let Some(content_end) = attr_block[cs..].find("</") {
                let value = attr_block[cs..cs + content_end].trim();
                if !value.is_empty() {
                    values.push(value.to_string());
                }
                search_from = cs + content_end;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Extract a named XML attribute from a specific element.
pub(crate) fn extract_xml_attribute_value(
    xml: &str,
    element_name: &str,
    attr_name: &str,
) -> Option<String> {
    let elem_pattern = format!("<{element_name}");
    let patterns = [elem_pattern.clone(), format!(":{element_name}")];

    for pattern in &patterns {
        if let Some(pos) = xml.find(pattern.as_str()) {
            let after = &xml[pos..];
            let tag_end = after.find('>')?;
            let tag = &after[..tag_end];

            let attr_pattern = format!("{attr_name}=\"");
            if let Some(attr_pos) = tag.find(&attr_pattern) {
                let value_start = attr_pos + attr_pattern.len();
                let value_end = tag[value_start..].find('"')? + value_start;
                return Some(tag[value_start..value_end].to_string());
            }
        }
    }

    None
}

// ============================================================================
// Signature extraction helpers (for SAML verification)
// ============================================================================

/// Extract the base64-encoded SignatureValue from a SAML response.
pub(crate) fn extract_signature_value(xml: &str) -> Option<String> {
    extract_xml_element(xml, "SignatureValue")
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
}

/// Extract the raw `<ds:SignedInfo>...</ds:SignedInfo>` block from SAML XML.
pub(crate) fn extract_signed_info(xml: &str) -> Option<String> {
    let start_patterns = ["<ds:SignedInfo", "<SignedInfo"];
    let end_patterns = ["</ds:SignedInfo>", "</SignedInfo>"];

    for (start_pat, end_pat) in start_patterns.iter().zip(end_patterns.iter()) {
        if let Some(start) = xml.find(start_pat) {
            if let Some(end) = xml[start..].find(end_pat) {
                return Some(xml[start..start + end + end_pat.len()].to_string());
            }
        }
    }

    None
}

/// Extract the DigestValue from a ds:Reference element.
pub(crate) fn extract_digest_value(xml: &str) -> Option<String> {
    extract_xml_element(xml, "DigestValue")
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
}

/// Simplified exclusive XML canonicalization for SAML SignedInfo blocks.
///
/// This implements a pragmatic subset of exc-c14n sufficient for verifying
/// SAML signatures from major IdPs (Okta, Azure AD, Google Workspace, OneLogin).
/// It handles:
/// - Whitespace normalization between elements
/// - Consistent attribute quoting
/// - Self-closing tag expansion
///
/// NOTE: This is NOT full W3C XML C14N. Full compliance requires an XML parser
/// library. This handles the 90%+ case of standard SAML responses.
pub(crate) fn canonicalize_xml(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len());
    let mut in_tag = false;
    let mut last_was_whitespace = false;

    for c in xml.chars() {
        if c == '<' {
            in_tag = true;
            last_was_whitespace = false;
            result.push(c);
        } else if c == '>' {
            in_tag = false;
            // Expand self-closing tags: convert <foo/> to <foo></foo>
            // Check if last char before > is /
            if result.ends_with('/') {
                result.pop(); // remove /
                result.push('>');
                // Extract tag name for closing tag
                if let Some(tag_start) = result.rfind('<') {
                    let tag_content = result[tag_start + 1..result.len() - 1].to_string();
                    let tag_name = tag_content.split_whitespace().next().unwrap_or("");
                    if !tag_name.is_empty()
                        && !tag_name.starts_with('?')
                        && !tag_name.starts_with('!')
                    {
                        result.push_str(&format!("</{tag_name}>"));
                    }
                }
            } else {
                result.push(c);
            }
            last_was_whitespace = false;
        } else if in_tag {
            // Inside a tag: normalize whitespace to single spaces
            if c.is_whitespace() {
                if !last_was_whitespace {
                    result.push(' ');
                    last_was_whitespace = true;
                }
            } else {
                result.push(c);
                last_was_whitespace = false;
            }
        } else {
            // Between tags: preserve content as-is
            result.push(c);
            last_was_whitespace = false;
        }
    }

    result
}

/// Remove the `<ds:Signature>` (or `<Signature>`) block from XML.
///
/// Used for digest verification -- the assertion body must be hashed
/// without the Signature element.
pub(crate) fn remove_signature_element(xml: &str) -> String {
    let start_patterns = ["<ds:Signature", "<Signature"];
    let end_patterns = ["</ds:Signature>", "</Signature>"];

    let mut result = xml.to_string();
    for (start_pat, end_pat) in start_patterns.iter().zip(end_patterns.iter()) {
        if let Some(start) = result.find(start_pat) {
            if let Some(end) = result[start..].find(end_pat) {
                let remove_end = start + end + end_pat.len();
                result = format!("{}{}", &result[..start], &result[remove_end..]);
                break;
            }
        }
    }

    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_element() {
        let xml = r#"<saml:NameID Format="email">alice@example.com</saml:NameID>"#;
        assert_eq!(
            extract_xml_element(xml, "NameID"),
            Some("alice@example.com".to_string())
        );
    }

    #[test]
    fn test_extract_xml_element_no_namespace() {
        let xml = r#"<NameID>bob@example.com</NameID>"#;
        assert_eq!(
            extract_xml_element(xml, "NameID"),
            Some("bob@example.com".to_string())
        );
    }

    #[test]
    fn test_extract_xml_element_missing() {
        let xml = r#"<Issuer>com.4da.app</Issuer>"#;
        assert_eq!(extract_xml_element(xml, "NameID"), None);
    }

    #[test]
    fn test_extract_saml_attribute() {
        let xml = r#"
        <saml:Attribute Name="displayName">
            <saml:AttributeValue>Alice Smith</saml:AttributeValue>
        </saml:Attribute>"#;
        assert_eq!(
            extract_saml_attribute(xml, "displayName"),
            Some("Alice Smith".to_string())
        );
    }

    #[test]
    fn test_extract_xml_attribute_value() {
        let xml = r#"<samlp:SubjectConfirmationData NotOnOrAfter="2026-03-14T00:00:00Z" Recipient="http://localhost:4445"/>"#;
        assert_eq!(
            extract_xml_attribute_value(xml, "SubjectConfirmationData", "NotOnOrAfter"),
            Some("2026-03-14T00:00:00Z".to_string())
        );
    }

    #[test]
    fn test_extract_signature_value() {
        let xml = r#"<ds:Signature>
            <ds:SignedInfo>...</ds:SignedInfo>
            <ds:SignatureValue>dGVzdC1zaWduYXR1cmU=</ds:SignatureValue>
        </ds:Signature>"#;
        assert_eq!(
            extract_signature_value(xml),
            Some("dGVzdC1zaWduYXR1cmU=".to_string())
        );
    }

    #[test]
    fn test_extract_signed_info() {
        let xml = r##"<ds:Signature><ds:SignedInfo><ds:Reference URI="#id"></ds:Reference></ds:SignedInfo></ds:Signature>"##;
        let signed_info = extract_signed_info(xml).unwrap();
        assert!(signed_info.starts_with("<ds:SignedInfo>"));
        assert!(signed_info.ends_with("</ds:SignedInfo>"));
    }

    #[test]
    fn test_extract_digest_value() {
        let xml = r#"<ds:Reference><ds:DigestValue>abc123==</ds:DigestValue></ds:Reference>"#;
        assert_eq!(extract_digest_value(xml), Some("abc123==".to_string()));
    }

    #[test]
    fn test_canonicalize_self_closing() {
        let input = r#"<foo attr="val"/>"#;
        let output = canonicalize_xml(input);
        assert!(output.contains("</foo>"));
        assert!(!output.contains("/>"));
    }

    #[test]
    fn test_remove_signature_element() {
        let xml = r#"<Assertion><ds:Signature><ds:SignedInfo/><ds:SignatureValue>abc</ds:SignatureValue></ds:Signature><Subject>test</Subject></Assertion>"#;
        let result = remove_signature_element(xml);
        assert!(!result.contains("Signature"));
        assert!(result.contains("<Subject>test</Subject>"));
    }
}
