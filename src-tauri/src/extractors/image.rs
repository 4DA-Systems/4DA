/// Image OCR extraction
///
/// Extracts text from images using OCR via the ocrs crate (pure Rust).
/// Requires model files in src-tauri/models/:
///   - text-detection.rten (~2.5MB)
///   - text-recognition.rten (~9.5MB)
///
/// Models can be downloaded from:
/// https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten
/// https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use crate::error::Result;
use image::ImageReader;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Global OCR engine instance (expensive to create, reuse across calls)
static OCR_ENGINE: OnceLock<std::result::Result<OcrEngine, String>> = OnceLock::new();

/// Find the models directory
fn find_models_dir() -> Option<PathBuf> {
    // Check common locations in order of preference
    let search_paths = [
        // 1. Relative to CARGO_MANIFEST_DIR (for development)
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")),
        // 2. Data directory alongside the app
        dirs::data_local_dir().map(|p| p.join("4da").join("models")),
        // 3. Current directory
        Some(PathBuf::from("models")),
    ];

    for path_opt in search_paths.iter().flatten() {
        let detection_path = path_opt.join("text-detection.rten");
        let recognition_path = path_opt.join("text-recognition.rten");
        if detection_path.exists() && recognition_path.exists() {
            return Some(path_opt.clone());
        }
    }
    None
}

/// Get or initialize the OCR engine (lazy singleton)
fn get_ocr_engine() -> std::result::Result<&'static OcrEngine, String> {
    let engine = OCR_ENGINE.get_or_init(|| {
        let models_dir = find_models_dir().ok_or_else(|| {
            "OCR models not found. Please download:\n\
             - https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten\n\
             - https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten\n\
             Place them in src-tauri/models/ or %LOCALAPPDATA%\\4da\\models\\"
                .to_string()
        })?;

        let detection_path = models_dir.join("text-detection.rten");
        let recognition_path = models_dir.join("text-recognition.rten");

        let detection_model = Model::load_file(&detection_path)
            .map_err(|e| format!("Failed to load detection model: {}", e))?;
        let recognition_model = Model::load_file(&recognition_path)
            .map_err(|e| format!("Failed to load recognition model: {}", e))?;

        let params = OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        };

        OcrEngine::new(params).map_err(|e| format!("Failed to initialize OCR engine: {}", e))
    });

    match engine {
        Ok(e) => Ok(e),
        Err(e) => Err(e.clone()),
    }
}

pub struct ImageExtractor;

impl ImageExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImageExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for ImageExtractor {
    fn supported_extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg", "tiff", "tif", "bmp", "gif", "webp"]
    }

    fn extract(&self, path: &Path) -> Result<ExtractedDocument> {
        // Load the image
        let img = ImageReader::open(path)
            .map_err(|e| format!("Failed to open image: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        // Convert to RGB8 format for OCR
        let rgb_img = img.to_rgb8();
        let (width, height) = rgb_img.dimensions();

        // Create image source for OCR engine
        let img_source = ImageSource::from_bytes(rgb_img.as_raw(), (width, height))
            .map_err(|e| format!("Failed to create image source: {}", e))?;

        // Get OCR engine and run recognition
        let engine = get_ocr_engine()?;
        let ocr_input = engine
            .prepare_input(img_source)
            .map_err(|e| format!("Failed to prepare OCR input: {}", e))?;

        // Detect text lines
        let word_rects = engine
            .detect_words(&ocr_input)
            .map_err(|e| format!("Failed to detect words: {}", e))?;

        let line_rects = engine.find_text_lines(&ocr_input, &word_rects);

        // Recognize text from detected lines
        let line_texts: Vec<String> = engine
            .recognize_text(&ocr_input, &line_rects)
            .map_err(|e| format!("Failed to recognize text: {}", e))?
            .iter()
            .filter_map(|line| line.as_ref().map(|l| l.to_string()))
            .collect();

        let text = line_texts.join("\n");

        // Calculate confidence based on whether we found text
        let confidence = if text.trim().is_empty() {
            0.0 // No text found
        } else if line_texts.len() > 3 {
            0.85 // Good amount of text detected
        } else {
            0.7 // Some text detected
        };

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("width".to_string(), width.to_string());
        metadata.insert("height".to_string(), height.to_string());
        metadata.insert("lines_detected".to_string(), line_texts.len().to_string());

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            metadata.insert("filename".to_string(), filename.to_string());
        }

        Ok(ExtractedDocument {
            text,
            metadata,
            pages: vec![PageContent {
                page_number: 1,
                text: line_texts.join("\n"),
                confidence: Some(confidence),
            }],
            confidence,
            source_type: "image".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_supported_extensions() {
        let extractor = ImageExtractor::new();
        let exts = extractor.supported_extensions();
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"jpeg"));
        assert!(exts.contains(&"tiff"));
        assert!(exts.contains(&"bmp"));
        assert!(exts.contains(&"webp"));
    }

    #[test]
    fn test_image_can_handle() {
        let extractor = ImageExtractor::new();
        assert!(extractor.can_handle(Path::new("test.png")));
        assert!(extractor.can_handle(Path::new("test.JPG")));
        assert!(extractor.can_handle(Path::new("test.JPEG")));
        assert!(!extractor.can_handle(Path::new("test.pdf")));
        assert!(!extractor.can_handle(Path::new("test.docx")));
    }

    #[test]
    fn test_image_nonexistent_file() {
        let extractor = ImageExtractor::new();
        let result = extractor.extract(Path::new("/nonexistent/image.png"));
        assert!(result.is_err());
    }

    #[test]
    fn test_find_models_dir() {
        // This test checks if models directory lookup works
        let result = find_models_dir();
        // Result may be Some or None depending on whether models are installed
        if let Some(path) = result {
            assert!(path.join("text-detection.rten").exists());
            assert!(path.join("text-recognition.rten").exists());
        }
    }
}
