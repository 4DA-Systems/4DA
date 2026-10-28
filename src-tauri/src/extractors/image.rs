// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/// Image OCR extraction
///
/// Extracts text from images using OCR via the ocrs crate (pure Rust).
/// Requires the ONNX model files in src-tauri/models/ (`pnpm install` fetches
/// and SHA-256-verifies them via scripts/fetch-ocr-models.cjs):
///   - text-detection.onnx (~2.5MB)
///   - text-recognition.onnx (~9.5MB)
///
/// Models can be downloaded from:
/// https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx
/// https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx
///
/// rten picks the model format from the file extension, which is why these are
/// loaded by their `.onnx` names. The `.rten` variants this used before are
/// deprecated upstream and their URLs return HTTP 403 since 2026-10-27
/// (robertknight/ocrs#282).
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use crate::error::{Result, ResultExt};
use image::ImageReader;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Text-detection model file name inside the models directory.
const DETECTION_MODEL: &str = "text-detection.onnx";
/// Text-recognition model file name inside the models directory.
const RECOGNITION_MODEL: &str = "text-recognition.onnx";

/// Global OCR engine instance (expensive to create, reuse across calls)
static OCR_ENGINE: OnceLock<std::result::Result<OcrEngine, String>> = OnceLock::new();

/// Find the models directory
fn find_models_dir() -> Option<PathBuf> {
    // Check common locations in order of preference
    let search_paths = [
        // 1. Via centralized RuntimePaths (dev: src-tauri/models, prod: resource_dir/models)
        Some(crate::runtime_paths::RuntimePaths::get().ocr_models_dir()),
        // 2. Data directory alongside the app
        dirs::data_local_dir().map(|p| p.join("4da").join("models")),
        // 3. Current directory
        Some(PathBuf::from("models")),
    ];

    for path_opt in search_paths.iter().flatten() {
        let detection_path = path_opt.join(DETECTION_MODEL);
        let recognition_path = path_opt.join(RECOGNITION_MODEL);
        if detection_path.exists() && recognition_path.exists() {
            return Some(path_opt.clone());
        }
    }
    None
}

/// Get or initialize the OCR engine (lazy singleton)
fn get_ocr_engine() -> crate::error::Result<&'static OcrEngine> {
    let engine = OCR_ENGINE.get_or_init(|| {
        let models_dir = find_models_dir().ok_or_else(|| {
            "OCR models not found. Run `node scripts/fetch-ocr-models.cjs`, or download:\n\
             - https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx\n\
             - https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx\n\
             Place them in src-tauri/models/ or your local data directory under 4da/models/"
                .to_string()
        })?;

        let detection_path = models_dir.join(DETECTION_MODEL);
        let recognition_path = models_dir.join(RECOGNITION_MODEL);

        let detection_model = Model::load_file(&detection_path)
            .map_err(|e| format!("Failed to load detection model: {e}"))?;
        let recognition_model = Model::load_file(&recognition_path)
            .map_err(|e| format!("Failed to load recognition model: {e}"))?;

        let params = OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        };

        OcrEngine::new(params).map_err(|e| format!("Failed to initialize OCR engine: {e}"))
    });

    match engine {
        Ok(e) => Ok(e),
        Err(e) => Err(e.clone().into()),
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
            .context("Failed to open image")?
            .decode()
            .context("Failed to decode image")?;

        // Convert to RGB8 format for OCR
        let rgb_img = img.to_rgb8();
        let (width, height) = rgb_img.dimensions();

        // Create image source for OCR engine
        let img_source = ImageSource::from_bytes(rgb_img.as_raw(), (width, height))
            .context("Failed to create image source")?;

        // Get OCR engine and run recognition
        let engine = get_ocr_engine()?;
        let ocr_input = engine
            .prepare_input(img_source)
            .context("Failed to prepare OCR input")?;

        // Detect text lines
        let word_rects = engine
            .detect_words(&ocr_input)
            .context("Failed to detect words")?;

        let line_rects = engine.find_text_lines(&ocr_input, &word_rects);

        // Recognize text from detected lines
        let line_texts: Vec<String> = engine
            .recognize_text(&ocr_input, &line_rects)
            .context("Failed to recognize text")?
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
            assert!(path.join(DETECTION_MODEL).exists());
            assert!(path.join(RECOGNITION_MODEL).exists());
        }
    }

    /// 5x7 bitmap glyphs (one byte per row, low 5 bits, MSB on the left).
    fn glyph(c: char) -> [u8; 7] {
        match c {
            'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
            'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
            'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
            'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
            'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
            'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
            'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
            _ => [0; 7],
        }
    }

    /// Black-on-white PNG of `text`, each bitmap pixel drawn as a `scale` square.
    fn render_text_png(text: &str, scale: u32, dest: &Path) {
        let margin = 4 * scale;
        // Two blank columns between glyphs: with one, "LL" reads as a single L.
        let advance = 7 * scale;
        let width = margin * 2 + advance * text.chars().count() as u32;
        let height = margin * 2 + 7 * scale;
        let mut img = image::RgbImage::from_pixel(width, height, image::Rgb([255, 255, 255]));
        for (i, c) in text.chars().enumerate() {
            let x0 = margin + advance * i as u32;
            for (row, bits) in glyph(c).iter().enumerate() {
                for col in 0..5u32 {
                    if bits & (0x10 >> col) == 0 {
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let x = x0 + col * scale + dx;
                            let y = margin + row as u32 * scale + dy;
                            img.put_pixel(x, y, image::Rgb([0, 0, 0]));
                        }
                    }
                }
            }
        }
        img.save(dest).unwrap();
    }

    /// End to end: load both real ONNX models and read text back from a
    /// generated image. Ignored by default because it needs the downloaded
    /// models. Run with `node scripts/fetch-ocr-models.cjs`, then
    /// `cargo test --features ocr --lib extractors::image -- --ignored`.
    #[test]
    #[ignore = "needs the OCR models in src-tauri/models (node scripts/fetch-ocr-models.cjs)"]
    fn test_onnx_models_load_and_read_generated_text() {
        assert!(
            find_models_dir().is_some(),
            "{DETECTION_MODEL} and {RECOGNITION_MODEL} not found; run node scripts/fetch-ocr-models.cjs"
        );
        get_ocr_engine().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("hello-world.png");
        render_text_png("HELLO WORLD", 6, &png);

        let doc = ImageExtractor::new().extract(&png).unwrap();
        println!("OCR read: {:?} (confidence {})", doc.text, doc.confidence);
        let letters: String = doc
            .text
            .to_uppercase()
            .chars()
            .filter(char::is_ascii_alphabetic)
            .collect();
        assert_eq!(letters, "HELLOWORLD", "OCR read {:?}", doc.text);
    }
}
