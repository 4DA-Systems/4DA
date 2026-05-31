// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/// PDF document extraction
///
/// Extracts text from PDF files using the pdf-extract crate.
/// Handles text-based PDFs (embedded fonts). Scanned PDFs
/// will be handled via OCR in the image extractor.
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use crate::error::{Result, ResultExt};
use crate::utils::sanitize_path;
use lopdf::Document;
use std::collections::HashMap;
use std::path::Path;

pub struct PdfExtractor;

impl PdfExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract text from a PDF file using pdf-extract.
    ///
    /// `pdf_extract` (and the `lopdf` it builds on) can *panic* — not merely
    /// return `Err` — on malformed, encrypted, or otherwise hostile PDFs.
    /// Extraction runs synchronously inside the file-watcher callback
    /// (`ace::process_file_changes`), so an unguarded panic would unwind into
    /// and kill that thread, silently stopping file monitoring for the session.
    /// We contain the panic with `catch_unwind` and degrade to a normal `Err`,
    /// so a single bad PDF can never crash extraction.
    fn extract_text(&self, path: &Path) -> Result<String> {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::extract_text(path)
        }));
        match outcome {
            Ok(result) => result.context("Failed to extract text from PDF"),
            Err(_) => {
                tracing::warn!(
                    target: "4da::extractors",
                    path = %sanitize_path(&path.to_string_lossy()),
                    "PDF text extraction panicked (malformed/encrypted PDF) — skipping file"
                );
                Err("PDF text extraction panicked (malformed or unsupported PDF)".into())
            }
        }
    }

    /// Extract metadata from a PDF using lopdf.
    ///
    /// `lopdf` can panic anywhere across `load` / `get_pages` / `get_object` on
    /// corrupt input, so the entire best-effort block is contained in a single
    /// `catch_unwind`. Metadata is non-essential: on any panic we return empty
    /// metadata rather than aborting extraction.
    fn extract_metadata(&self, path: &Path) -> HashMap<String, String> {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::extract_metadata_inner(path)
        }));
        match outcome {
            Ok(metadata) => metadata,
            Err(_) => {
                tracing::warn!(
                    target: "4da::extractors",
                    path = %sanitize_path(&path.to_string_lossy()),
                    "PDF metadata extraction panicked — returning empty metadata"
                );
                HashMap::new()
            }
        }
    }

    /// The lopdf-backed metadata extraction body. Separated so the whole block
    /// can be wrapped in `catch_unwind` by [`Self::extract_metadata`].
    fn extract_metadata_inner(path: &Path) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        // Try to load PDF with lopdf for metadata
        if let Ok(doc) = Document::load(path) {
            // Get page count
            let page_count = doc.get_pages().len();
            metadata.insert("page_count".to_string(), page_count.to_string());

            // Try to extract document info dictionary
            if let Ok(info_dict) = doc.trailer.get(b"Info") {
                if let Ok(info_ref) = info_dict.as_reference() {
                    if let Ok(info) = doc.get_object(info_ref) {
                        if let Ok(dict) = info.as_dict() {
                            // Extract common metadata fields
                            for key in [
                                "Title",
                                "Author",
                                "Subject",
                                "Creator",
                                "Producer",
                                "CreationDate",
                                "ModDate",
                            ] {
                                if let Ok(value) = dict.get(key.as_bytes()) {
                                    // Try different lopdf object types
                                    if let Ok(bytes) = value.as_str() {
                                        // as_str returns &[u8], convert to String
                                        let text = String::from_utf8_lossy(bytes).to_string();
                                        if !text.is_empty() {
                                            metadata.insert(key.to_lowercase(), text);
                                        }
                                    } else if let Ok(text) = value.as_name_str() {
                                        metadata.insert(key.to_lowercase(), text.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        metadata
    }

    /// Split text into pages (best effort based on form feeds or page markers)
    fn split_into_pages(&self, text: &str, page_count: usize) -> Vec<PageContent> {
        // If we have a page count from metadata, try to split evenly
        // This is a heuristic since pdf-extract doesn't provide page boundaries

        // First, try splitting on form feed characters (common PDF page separator)
        let raw_pages: Vec<&str> = text.split('\x0C').collect();

        if raw_pages.len() > 1 {
            // Form feed separators found
            return raw_pages
                .into_iter()
                .enumerate()
                .filter(|(_, content)| !content.trim().is_empty())
                .map(|(i, content)| PageContent {
                    page_number: i + 1,
                    text: content.trim().to_string(),
                    confidence: Some(1.0),
                })
                .collect();
        }

        // No form feeds - try to estimate pages based on double newlines
        let paragraphs: Vec<&str> = text.split("\n\n").collect();

        if page_count > 1 && paragraphs.len() >= page_count {
            // Distribute paragraphs across pages
            let paras_per_page = paragraphs.len() / page_count;
            let mut pages = Vec::new();

            for (page_idx, chunk) in paragraphs.chunks(paras_per_page).enumerate() {
                let page_text = chunk.join("\n\n").trim().to_string();
                if !page_text.is_empty() {
                    pages.push(PageContent {
                        page_number: page_idx + 1,
                        text: page_text,
                        confidence: Some(1.0),
                    });
                }
            }

            return pages;
        }

        // Single page or couldn't split - return as single page
        if text.trim().is_empty() {
            vec![]
        } else {
            vec![PageContent {
                page_number: 1,
                text: text.trim().to_string(),
                confidence: Some(1.0),
            }]
        }
    }
}

impl Default for PdfExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for PdfExtractor {
    fn supported_extensions(&self) -> &[&str] {
        &["pdf"]
    }

    fn extract(&self, path: &Path) -> Result<ExtractedDocument> {
        // Verify file exists and is readable
        if !path.exists() {
            return Err(format!(
                "File does not exist: {}",
                sanitize_path(&path.to_string_lossy())
            )
            .into());
        }

        // Extract text content (cap at 5MB to prevent memory exhaustion)
        let text = self.extract_text(path)?;
        let text = if text.len() > 5_000_000 {
            tracing::warn!(target: "4da::extractors", bytes = text.len(), "PDF text exceeds 5MB limit — truncating");
            text[..5_000_000].to_string()
        } else {
            text
        };

        // If no text extracted, this might be a scanned PDF
        if text.trim().is_empty() {
            return Err(format!(
                "No text content found in PDF (may be scanned/image-only): {}",
                sanitize_path(&path.to_string_lossy())
            )
            .into());
        }

        // Extract metadata
        let metadata = self.extract_metadata(path);

        // Get page count from metadata or default to 1
        let page_count: usize = metadata
            .get("page_count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        // Split into pages
        let pages = self.split_into_pages(&text, page_count);

        Ok(ExtractedDocument {
            text: text.trim().to_string(),
            metadata,
            pages,
            confidence: 1.0, // Text-based PDFs have high confidence
            source_type: "pdf".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_pdf_supported_extensions() {
        let extractor = PdfExtractor::new();
        assert_eq!(extractor.supported_extensions(), &["pdf"]);
    }

    #[test]
    fn test_pdf_can_handle() {
        let extractor = PdfExtractor::new();
        assert!(extractor.can_handle(Path::new("test.pdf")));
        assert!(extractor.can_handle(Path::new("test.PDF")));
        assert!(!extractor.can_handle(Path::new("test.docx")));
        assert!(!extractor.can_handle(Path::new("test.txt")));
    }

    #[test]
    fn test_pdf_nonexistent_file() {
        let extractor = PdfExtractor::new();
        let result = extractor.extract(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_split_into_pages_form_feed() {
        let extractor = PdfExtractor::new();
        let text = "Page 1 content\x0CPage 2 content\x0CPage 3 content";
        let pages = extractor.split_into_pages(text, 3);

        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].page_number, 1);
        assert!(pages[0].text.contains("Page 1"));
        assert_eq!(pages[2].page_number, 3);
        assert!(pages[2].text.contains("Page 3"));
    }

    #[test]
    fn test_split_into_pages_single() {
        let extractor = PdfExtractor::new();
        let text = "Single page content without separators";
        let pages = extractor.split_into_pages(text, 1);

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_number, 1);
    }

    /// A file with a valid PDF header but a garbage body must surface as an
    /// `Err`, never a panic — this exercises the `catch_unwind` guards around
    /// the third-party `pdf_extract` / `lopdf` calls.
    #[test]
    fn test_corrupt_pdf_does_not_panic() {
        let extractor = PdfExtractor::new();
        let dir = std::env::temp_dir().join("4da_pdf_corrupt_test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrupt.pdf");
        // Looks like a PDF (header) but the body is hostile binary garbage.
        fs::write(
            &path,
            b"%PDF-1.7\n\xff\xff\xde\xad\xbe\xef\x00\x01 not a real pdf body \xc0\xc1",
        )
        .unwrap();

        // Must return Err (or empty-text Err), never unwind into the caller.
        let result = extractor.extract(&path);
        assert!(result.is_err(), "corrupt PDF should error, not panic");

        // Metadata extraction on the same garbage must also stay contained.
        let metadata = extractor.extract_metadata(&path);
        let _ = metadata; // empty or partial is fine; the point is no panic

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_metadata_extraction_defaults() {
        let extractor = PdfExtractor::new();
        // When file doesn't exist, should return empty metadata
        let metadata = extractor.extract_metadata(Path::new("/nonexistent/file.pdf"));
        assert!(metadata.is_empty());
    }

    // Integration test - requires a real PDF file
    #[test]
    #[ignore] // Run with: cargo test test_real_pdf -- --ignored
    fn test_real_pdf_extraction() {
        // Create a minimal PDF for testing
        let test_dir = std::env::temp_dir().join("4da_pdf_test");
        fs::create_dir_all(&test_dir).unwrap();

        // Note: This test requires a real PDF file
        // To test locally, place a PDF at the test path and run with --ignored
        let test_pdf = test_dir.join("test.pdf");

        if test_pdf.exists() {
            let extractor = PdfExtractor::new();
            let result = extractor.extract(&test_pdf);

            assert!(result.is_ok(), "Should extract PDF: {:?}", result.err());
            let doc = result.unwrap();
            assert!(!doc.text.is_empty(), "Should have extracted text");
            assert_eq!(doc.source_type, "pdf");
            assert_eq!(doc.confidence, 1.0);
        }
    }
}
