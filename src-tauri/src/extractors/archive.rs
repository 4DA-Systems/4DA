/// Archive extraction (ZIP, TAR, etc.)
///
/// Recursively extracts and processes files from archive formats.
/// Prevents zip bombs with depth and size limits.
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use crate::error::{Result, ResultExt};
use crate::utils::sanitize_path;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tar::Archive as TarArchive;
use zip::ZipArchive;

/// Security limits for archive extraction
const MAX_DEPTH: u32 = 3;
const MAX_EXTRACTED_SIZE: u64 = 100 * 1024 * 1024; // 100MB total
const MAX_FILE_COUNT: usize = 1000;
const MAX_SINGLE_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB per file
const MAX_COMPRESSED_SIZE: u64 = 50 * 1024 * 1024; // 50MB compressed input
const MAX_COMPRESSION_RATIO: u64 = 100; // Abort if ratio > 100:1 (decompression bomb)

pub struct ArchiveExtractor;

impl ArchiveExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract a ZIP archive
    fn extract_zip(&self, path: &Path) -> Result<ExtractedDocument> {
        // Check compressed file size to prevent decompression bombs
        let compressed_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if compressed_size > MAX_COMPRESSED_SIZE {
            return Err(format!(
                "Archive too large: {}MB exceeds {}MB limit",
                compressed_size / (1024 * 1024),
                MAX_COMPRESSED_SIZE / (1024 * 1024)
            )
            .into());
        }

        let file = File::open(path).context("Failed to open ZIP")?;
        let mut archive = ZipArchive::new(file).context("Failed to read ZIP archive")?;

        let mut all_text = Vec::new();
        let mut metadata = HashMap::new();
        let mut total_size: u64 = 0;
        let mut file_count = 0;

        metadata.insert("archive_type".to_string(), "zip".to_string());
        metadata.insert("file_count".to_string(), archive.len().to_string());

        for i in 0..archive.len() {
            if file_count >= MAX_FILE_COUNT {
                break;
            }

            let mut file = archive.by_index(i).context("Failed to read ZIP entry")?;

            // Security: Check for path traversal
            let name = match file.enclosed_name() {
                Some(name) => name.to_path_buf(),
                None => continue, // Skip files with invalid paths
            };

            // Skip directories
            if file.is_dir() {
                continue;
            }

            // Check depth
            if name.components().count() > MAX_DEPTH as usize {
                continue;
            }

            // Check size + decompression bomb ratio
            let size = file.size();
            if size > MAX_SINGLE_FILE_SIZE {
                continue;
            }
            if total_size + size > MAX_EXTRACTED_SIZE {
                break;
            }
            let compressed = file.compressed_size();
            if compressed > 0 && size / compressed > MAX_COMPRESSION_RATIO {
                tracing::warn!(
                    target: "4da::extract",
                    "Skipping suspicious entry '{}': compression ratio {}:1 exceeds limit",
                    name.display(), size / compressed
                );
                continue;
            }

            // Read content
            let mut content = Vec::new();
            if file.read_to_end(&mut content).is_ok() {
                total_size += content.len() as u64;
                file_count += 1;

                // Try to extract text based on extension
                if let Some(text) = self.extract_text_from_content(&name, &content) {
                    all_text.push(format!("=== {} ===\n{}", name.display(), text));
                }
            }
        }

        metadata.insert("extracted_files".to_string(), file_count.to_string());
        metadata.insert("total_size".to_string(), total_size.to_string());

        let full_text = all_text.join("\n\n");

        if full_text.trim().is_empty() {
            return Err("No extractable text content found in archive".into());
        }

        Ok(ExtractedDocument {
            text: full_text,
            metadata,
            pages: vec![PageContent {
                page_number: 1,
                text: format!("Archive with {} files", file_count),
                confidence: Some(1.0),
            }],
            confidence: 1.0,
            source_type: "zip".to_string(),
        })
    }

    /// Extract a TAR archive (optionally compressed)
    fn extract_tar(&self, path: &Path) -> Result<ExtractedDocument> {
        let file = File::open(path).context("Failed to open TAR")?;

        // Detect compression based on extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let reader: Box<dyn Read> = match ext.as_str() {
            "gz" | "tgz" => Box::new(flate2::read::GzDecoder::new(file)),
            _ => Box::new(file),
        };

        let mut archive = TarArchive::new(reader);
        let entries = archive.entries().context("Failed to read TAR entries")?;

        let mut all_text = Vec::new();
        let mut metadata = HashMap::new();
        let mut total_size: u64 = 0;
        let mut file_count = 0;

        metadata.insert("archive_type".to_string(), "tar".to_string());

        for entry_result in entries {
            if file_count >= MAX_FILE_COUNT {
                break;
            }

            let mut entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let entry_path = match entry.path() {
                Ok(p) => p.to_path_buf(),
                Err(_) => continue,
            };

            // Security: Check for path traversal (absolute paths or ..)
            if entry_path.is_absolute()
                || entry_path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                continue;
            }

            // Skip directories
            if entry.header().entry_type().is_dir() {
                continue;
            }

            // Check depth
            if entry_path.components().count() > MAX_DEPTH as usize {
                continue;
            }

            // Check size
            let size = entry.header().size().unwrap_or(0);
            if size > MAX_SINGLE_FILE_SIZE {
                continue;
            }
            if total_size + size > MAX_EXTRACTED_SIZE {
                break;
            }

            // Read content
            let mut content = Vec::new();
            if entry.read_to_end(&mut content).is_ok() {
                total_size += content.len() as u64;
                file_count += 1;

                // Try to extract text based on extension
                if let Some(text) = self.extract_text_from_content(&entry_path, &content) {
                    all_text.push(format!("=== {} ===\n{}", entry_path.display(), text));
                }
            }
        }

        metadata.insert("extracted_files".to_string(), file_count.to_string());
        metadata.insert("total_size".to_string(), total_size.to_string());

        let full_text = all_text.join("\n\n");

        if full_text.trim().is_empty() {
            return Err("No extractable text content found in archive".into());
        }

        Ok(ExtractedDocument {
            text: full_text,
            metadata,
            pages: vec![PageContent {
                page_number: 1,
                text: format!("Archive with {} files", file_count),
                confidence: Some(1.0),
            }],
            confidence: 1.0,
            source_type: "tar".to_string(),
        })
    }

    /// Extract text from file content based on extension
    fn extract_text_from_content(&self, path: &Path, content: &[u8]) -> Option<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            // Text-based files
            "txt" | "md" | "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "json" | "toml" | "yaml"
            | "yml" | "xml" | "html" | "css" | "sh" | "bash" | "go" | "java" | "c" | "cpp"
            | "h" | "hpp" | "rb" | "php" | "sql" | "ini" | "cfg" | "conf" | "log" => {
                String::from_utf8(content.to_vec()).ok()
            }
            // Skip binary formats - would need nested extraction
            "pdf" | "docx" | "xlsx" | "zip" | "tar" | "gz" => None,
            // Skip images/media
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "mp3" | "mp4" | "wav" => None,
            // Try as text for unknown extensions
            _ => {
                // Only try if it looks like text (no null bytes in first 1000 chars)
                let sample = &content[..content.len().min(1000)];
                if sample.contains(&0) {
                    None
                } else {
                    String::from_utf8(content.to_vec()).ok()
                }
            }
        }
    }
}

impl Default for ArchiveExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for ArchiveExtractor {
    fn supported_extensions(&self) -> &[&str] {
        &["zip", "tar", "gz", "tgz"]
    }

    fn extract(&self, path: &Path) -> Result<ExtractedDocument> {
        if !path.exists() {
            return Err(format!(
                "File does not exist: {}",
                sanitize_path(&path.to_string_lossy())
            )
            .into());
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .ok_or_else(|| "File has no extension".to_string())?;

        match ext.as_str() {
            "zip" => self.extract_zip(path),
            "tar" | "gz" | "tgz" => self.extract_tar(path),
            _ => Err(format!("Unsupported archive format: {}", ext).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_archive_supported_extensions() {
        let extractor = ArchiveExtractor::new();
        let exts = extractor.supported_extensions();
        assert!(exts.contains(&"zip"));
        assert!(exts.contains(&"tar"));
        assert!(exts.contains(&"gz"));
        assert!(exts.contains(&"tgz"));
    }

    #[test]
    fn test_archive_can_handle() {
        let extractor = ArchiveExtractor::new();
        assert!(extractor.can_handle(Path::new("test.zip")));
        assert!(extractor.can_handle(Path::new("test.tar")));
        assert!(extractor.can_handle(Path::new("test.tar.gz")));
        assert!(extractor.can_handle(Path::new("test.tgz")));
        assert!(!extractor.can_handle(Path::new("test.pdf")));
    }

    #[test]
    fn test_archive_nonexistent_file() {
        let extractor = ArchiveExtractor::new();
        let result = extractor.extract(Path::new("/nonexistent/file.zip"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_extract_text_from_content_text_file() {
        let extractor = ArchiveExtractor::new();
        let content = b"Hello, World!";
        let result = extractor.extract_text_from_content(Path::new("test.txt"), content);
        assert_eq!(result, Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_extract_text_from_content_code_file() {
        let extractor = ArchiveExtractor::new();
        let content = b"fn main() { println!(\"Hello\"); }";
        let result = extractor.extract_text_from_content(Path::new("main.rs"), content);
        assert!(result.is_some());
        assert!(result.unwrap().contains("fn main"));
    }

    #[test]
    fn test_extract_text_from_content_binary() {
        let extractor = ArchiveExtractor::new();
        let content = &[0x00, 0x01, 0x02, 0x03]; // Binary with null byte
        let result = extractor.extract_text_from_content(Path::new("binary.dat"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_text_from_content_skips_nested_archives() {
        let extractor = ArchiveExtractor::new();
        let content = b"PK..."; // ZIP magic bytes don't matter, extension does
        let result = extractor.extract_text_from_content(Path::new("nested.zip"), content);
        assert!(result.is_none());
    }

    // Integration tests requiring real archives
    #[test]
    #[ignore]
    fn test_real_zip_extraction() {
        // Create a test ZIP in temp dir
        let test_dir = std::env::temp_dir().join("4da_archive_test");
        fs::create_dir_all(&test_dir).unwrap();
        let zip_path = test_dir.join("test.zip");

        // Create ZIP with test content
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("readme.txt", options).unwrap();
        zip.write_all(b"This is a test README file.").unwrap();

        zip.start_file("src/main.rs", options).unwrap();
        zip.write_all(b"fn main() { println!(\"Hello\"); }")
            .unwrap();

        zip.finish().unwrap();

        // Test extraction
        let extractor = ArchiveExtractor::new();
        let result = extractor.extract(&zip_path);
        assert!(result.is_ok(), "Should extract ZIP: {:?}", result.err());

        let doc = result.unwrap();
        assert!(doc.text.contains("test README"));
        assert!(doc.text.contains("fn main"));
        assert_eq!(doc.source_type, "zip");

        // Cleanup
        fs::remove_file(zip_path).ok();
    }
}
