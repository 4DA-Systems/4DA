use std::collections::HashMap;
/// Document extraction module for multi-format file support
///
/// This module provides a unified interface for extracting text and metadata
/// from various file formats including PDFs, Office documents, images (OCR),
/// audio files (transcription), and archives.
use std::path::Path;

// Re-export sub-modules
pub mod archive;
pub mod audio;
pub mod image;
pub mod office;
pub mod pdf;

/// Unified document extractor trait
///
/// All format-specific extractors implement this trait to provide
/// a consistent interface for the file indexing system.
pub trait DocumentExtractor: Send + Sync {
    /// Returns the file extensions this extractor can handle
    fn supported_extensions(&self) -> &[&str];

    /// Extract text and metadata from a file
    fn extract(&self, path: &Path) -> Result<ExtractedDocument, String>;

    /// Check if this extractor can handle a given file
    fn can_handle(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            self.supported_extensions()
                .contains(&ext.to_lowercase().as_str())
        } else {
            false
        }
    }
}

/// Extracted document content and metadata
#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    /// Extracted text content
    pub text: String,

    /// File-specific metadata (author, title, etc.)
    pub metadata: HashMap<String, String>,

    /// Multi-page documents (PDFs, presentations)
    pub pages: Vec<PageContent>,

    /// Extraction confidence (0.0-1.0)
    /// - 1.0 for text files, PDFs with embedded text
    /// - 0.0-1.0 for OCR (depends on image quality)
    /// - 0.8-1.0 for audio transcription (depends on Whisper model)
    pub confidence: f32,

    /// File format type
    pub source_type: String,
}

/// Content from a single page (for multi-page documents)
#[derive(Debug, Clone)]
pub struct PageContent {
    /// Page number (1-indexed)
    pub page_number: usize,

    /// Text content from this page
    pub text: String,

    /// Page-specific confidence (for OCR)
    pub confidence: Option<f32>,
}

impl ExtractedDocument {
    /// Create a new extracted document with default values
    pub fn new(text: String, source_type: String) -> Self {
        Self {
            text,
            metadata: HashMap::new(),
            pages: Vec::new(),
            confidence: 1.0,
            source_type,
        }
    }

    /// Check if extraction has high confidence
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.7
    }

    /// Get total word count
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// Extractor registry for managing all document extractors
pub struct ExtractorRegistry {
    extractors: Vec<Box<dyn DocumentExtractor>>,
}

impl ExtractorRegistry {
    /// Create a new registry with all available extractors
    pub fn new() -> Self {
        let mut registry = Self {
            extractors: Vec::new(),
        };

        // Register all extractors
        registry.register(Box::new(pdf::PdfExtractor::new()));
        registry.register(Box::new(office::OfficeExtractor::new()));
        registry.register(Box::new(image::ImageExtractor::new()));
        registry.register(Box::new(audio::AudioExtractor::new()));
        registry.register(Box::new(archive::ArchiveExtractor::new()));

        registry
    }

    /// Register a new extractor
    pub fn register(&mut self, extractor: Box<dyn DocumentExtractor>) {
        self.extractors.push(extractor);
    }

    /// Find an extractor that can handle the given file
    pub fn find_extractor(&self, path: &Path) -> Option<&dyn DocumentExtractor> {
        self.extractors
            .iter()
            .find(|e| e.can_handle(path))
            .map(|b| b.as_ref())
    }

    /// Extract content from a file using the appropriate extractor
    pub fn extract(&self, path: &Path) -> Result<ExtractedDocument, String> {
        let extractor = self
            .find_extractor(path)
            .ok_or_else(|| format!("No extractor found for file: {:?}", path))?;

        extractor.extract(path)
    }

    /// List all supported extensions
    pub fn supported_extensions(&self) -> Vec<String> {
        self.extractors
            .iter()
            .flat_map(|e| e.supported_extensions().iter().map(|s| s.to_string()))
            .collect()
    }
}

impl Default for ExtractorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_registry() {
        let registry = ExtractorRegistry::new();
        let extensions = registry.supported_extensions();

        // Should support PDF
        assert!(extensions.contains(&"pdf".to_string()));

        // Should support Office formats
        assert!(extensions.contains(&"docx".to_string()));

        // Should have multiple extractors registered
        assert!(registry.extractors.len() >= 5);
    }

    #[test]
    fn test_extracted_document_confidence() {
        let mut doc = ExtractedDocument::new("test content".to_string(), "text".to_string());

        // Default confidence should be high
        assert!(doc.is_high_confidence());

        // Low confidence should be detected
        doc.confidence = 0.5;
        assert!(!doc.is_high_confidence());
    }

    #[test]
    fn test_word_count() {
        let doc = ExtractedDocument::new("hello world test".to_string(), "text".to_string());
        assert_eq!(doc.word_count(), 3);
    }
}
