//! Test OCR extraction with a real image
//!
//! Run with: cargo run --bin test_ocr --release

use fourda_lib::extractors::{image::ImageExtractor, DocumentExtractor};
use std::path::Path;

fn main() {
    println!("=== OCR Test ===\n");

    // Test with images that exist in the project
    let test_images = [
        "../src/assets/sun-logo.jpg",
        "icons/icon.png",
        "icons/128x128.png",
    ];

    let extractor = ImageExtractor::new();

    for image_path in test_images {
        let path = Path::new(image_path);
        println!("Testing: {}", path.display());

        if !path.exists() {
            println!("  File not found, skipping\n");
            continue;
        }

        // Try extraction
        match extractor.extract(path) {
            Ok(doc) => {
                println!("  Confidence: {:.2}", doc.confidence);
                println!(
                    "  Lines detected: {}",
                    doc.pages
                        .first()
                        .map(|p| p.text.lines().count())
                        .unwrap_or(0)
                );
                if doc.text.trim().is_empty() {
                    println!("  Text: (no text detected - image may not contain text)");
                } else {
                    let preview: String = doc.text.chars().take(200).collect();
                    println!("  Text: {:?}", preview);
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
        println!();
    }

    println!("=== Done ===");
}
