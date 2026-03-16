use fourda_lib::extractors::{image::ImageExtractor, DocumentExtractor};
use std::path::Path;

fn main() {
    println!("=== OCR Test with Text Image ===\n");

    let path = Path::new("models/test-image.png");
    println!("Testing: {}", path.display());

    if !path.exists() {
        println!("File not found!");
        return;
    }

    let extractor = ImageExtractor::new();
    match extractor.extract(path) {
        Ok(doc) => {
            println!("Confidence: {:.2}", doc.confidence);
            println!(
                "Lines detected: {}",
                doc.pages
                    .first()
                    .map(|p| p.text.lines().count())
                    .unwrap_or(0)
            );
            println!("Text: {:?}", doc.text);
        }
        Err(e) => println!("Error: {}", e),
    }
}
