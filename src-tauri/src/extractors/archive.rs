// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

/// Entries *examined* before the walk gives up, whether or not they were read.
///
/// `MAX_FILE_COUNT` counts successfully-read files only, so an archive made
/// entirely of entries the walk skips (directories, over-depth paths,
/// over-size headers) never increments it and the loop runs once per entry
/// forever — on the file-watcher thread, holding the DB mutex.
const MAX_ENTRIES_SCANNED: usize = 100_000;

/// Read at most `budget` bytes from `reader`, returning `None` if the source
/// had more to give.
///
/// Header-declared sizes are attacker-controlled: a ZIP entry can declare 1 MB
/// and ship 45 MB of DEFLATE, and a TAR header can declare anything at all.
/// Every size decision in this module is made from what actually came out.
fn read_capped<R: Read>(reader: &mut R, budget: u64) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    // One byte past the budget, so an over-budget source is detectable rather
    // than silently truncated into a "valid" partial file.
    reader.take(budget + 1).read_to_end(&mut buf).ok()?;
    if buf.len() as u64 > budget {
        return None;
    }
    Some(buf)
}

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

        for i in 0..archive.len().min(MAX_ENTRIES_SCANNED) {
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

            // Cheap pre-filter on the declared size. This is a hint only —
            // `file.size()` comes from the ZIP header, which the archive's
            // author controls, so it can under-report by any factor. The
            // binding limit is the capped read below.
            if file.size() > MAX_SINGLE_FILE_SIZE {
                continue;
            }

            // Read content, bounded by whichever limit bites first.
            let budget = MAX_SINGLE_FILE_SIZE.min(MAX_EXTRACTED_SIZE.saturating_sub(total_size));
            if budget == 0 {
                break;
            }
            let content = match read_capped(&mut file, budget) {
                Some(c) => c,
                None => {
                    tracing::warn!(
                        target: "4da::extract",
                        entry = %name.display(),
                        declared = file.size(),
                        "Skipping ZIP entry: inflated past its declared size (decompression bomb)"
                    );
                    continue;
                }
            };

            // Decompression-bomb ratio, measured on the bytes that actually
            // came out rather than on the header's claim.
            let compressed = file.compressed_size();
            let actual = content.len() as u64;
            if compressed > 0 && actual / compressed > MAX_COMPRESSION_RATIO {
                tracing::warn!(
                    target: "4da::extract",
                    entry = %name.display(),
                    ratio = actual / compressed,
                    "Skipping suspicious entry: compression ratio exceeds limit"
                );
                continue;
            }

            total_size += actual;
            file_count += 1;

            // Try to extract text based on extension
            if let Some(text) = self.extract_text_from_content(&name, &content) {
                all_text.push(format!("=== {} ===\n{}", name.display(), text));
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
        // Bound the compressed input, as the ZIP path already did. A `.tar.gz`
        // had no input cap at all.
        let compressed_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if compressed_size > MAX_COMPRESSED_SIZE {
            return Err(format!(
                "Archive too large: {}MB exceeds {}MB limit",
                compressed_size / (1024 * 1024),
                MAX_COMPRESSED_SIZE / (1024 * 1024)
            )
            .into());
        }

        let file = File::open(path).context("Failed to open TAR")?;

        // Detect compression based on extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Bound the *decompressed* stream globally. A 50 MB gzip member can
        // inflate to terabytes; `Take` stops the inflater at the total budget
        // no matter what the tar headers claim, so nothing downstream — entry
        // walk included — can be driven past it.
        let reader: Box<dyn Read> = match ext.as_str() {
            "gz" | "tgz" => Box::new(
                flate2::read::GzDecoder::new(file).take(MAX_EXTRACTED_SIZE.saturating_add(1)),
            ),
            _ => Box::new(file.take(MAX_EXTRACTED_SIZE.saturating_add(1))),
        };

        let mut archive = TarArchive::new(reader);
        let entries = archive.entries().context("Failed to read TAR entries")?;

        let mut all_text = Vec::new();
        let mut metadata = HashMap::new();
        let mut total_size: u64 = 0;
        let mut file_count = 0;

        metadata.insert("archive_type".to_string(), "tar".to_string());

        let mut entries_scanned = 0usize;

        for entry_result in entries {
            if file_count >= MAX_FILE_COUNT {
                break;
            }

            // Every entry the walk *looks at* counts, not just the ones it
            // reads. Without this an archive of skippable entries (all
            // directories, all over-depth, all over-size) spins here forever
            // because `file_count` never moves.
            entries_scanned += 1;
            if entries_scanned > MAX_ENTRIES_SCANNED {
                tracing::warn!(
                    target: "4da::extract",
                    scanned = entries_scanned,
                    "Abandoning TAR walk: entry-scan limit reached"
                );
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

            // Cheap pre-filter on the declared size — a hint only, since the
            // TAR header is written by the archive's author.
            if entry.header().size().unwrap_or(0) > MAX_SINGLE_FILE_SIZE {
                continue;
            }

            // Read content, bounded by whichever limit bites first.
            let budget = MAX_SINGLE_FILE_SIZE.min(MAX_EXTRACTED_SIZE.saturating_sub(total_size));
            if budget == 0 {
                break;
            }
            let content = match read_capped(&mut entry, budget) {
                Some(c) => c,
                None => {
                    tracing::warn!(
                        target: "4da::extract",
                        entry = %entry_path.display(),
                        declared = entry.header().size().unwrap_or(0),
                        "Skipping TAR entry: read past its declared size (decompression bomb)"
                    );
                    continue;
                }
            };

            total_size += content.len() as u64;
            file_count += 1;

            // Try to extract text based on extension
            if let Some(text) = self.extract_text_from_content(&entry_path, &content) {
                all_text.push(format!("=== {} ===\n{}", entry_path.display(), text));
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

    // ================================================================
    // Real-archive integration tests
    //
    // Fixtures are built programmatically; nothing large is committed.
    // ================================================================

    /// Baseline: an honest ZIP still extracts. (Previously `#[ignore]`d for no
    /// stated reason — it builds its own fixture and needs nothing external.)
    #[test]
    fn test_real_zip_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");

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

        let doc = ArchiveExtractor::new()
            .extract(&zip_path)
            .expect("honest ZIP should extract");
        assert!(doc.text.contains("test README"));
        assert!(doc.text.contains("fn main"));
        assert_eq!(doc.source_type, "zip");
    }

    /// Overwrite the 4-byte little-endian *uncompressed size* in both the
    /// first local file header and the first central-directory record.
    ///
    /// This is what an attacker does by hand: `zip` 0.6 builds the DEFLATE
    /// reader as `DeflateDecoder::new(take(compressed_size))` (read.rs:277),
    /// so the declared uncompressed size bounds *nothing* — it is pure
    /// metadata, and every size decision that trusted `file.size()` trusted
    /// the attacker.
    fn forge_declared_size(path: &Path, lie: u32) {
        let mut bytes = fs::read(path).expect("read fixture");
        let lie = lie.to_le_bytes();

        // Local file header at offset 0: uncompressed size lives at +22.
        assert_eq!(&bytes[0..4], b"PK\x03\x04", "expected a local file header");
        bytes[22..26].copy_from_slice(&lie);

        // End of central directory: scan back for the signature, then read the
        // central directory offset out of it (+16).
        let eocd = (0..bytes.len().saturating_sub(21))
            .rev()
            .find(|&i| &bytes[i..i + 4] == b"PK\x05\x06")
            .expect("expected an end-of-central-directory record");
        let cd_off = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap()) as usize;

        // Central directory record: uncompressed size lives at +24.
        assert_eq!(
            &bytes[cd_off..cd_off + 4],
            b"PK\x01\x02",
            "expected a central directory record"
        );
        bytes[cd_off + 24..cd_off + 28].copy_from_slice(&lie);

        fs::write(path, bytes).expect("write forged fixture");
    }

    /// The ZIP decompression bomb: an entry that declares 1 KB and ships
    /// 45 MB of DEFLATE.
    ///
    /// Before the fix every guard in `extract_zip` read `file.size()` — the
    /// forged 1 KB — so the entry passed the per-file cap, passed the running
    /// total, passed the ratio check, and was then read with an unbounded
    /// `read_to_end`. The archive also carries one honest text file, so the
    /// extraction succeeds either way and the assertions can distinguish
    /// "bomb refused" from "archive rejected for some other reason".
    #[test]
    fn zip_entry_lying_about_its_size_is_refused() {
        const BOMB_MB: usize = 45;
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("bomb.zip");

        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(1));

            // Written first so its headers are the ones at the known offsets.
            zip.start_file("bomb.txt", options).unwrap();
            let chunk = vec![b'A'; 1024 * 1024];
            for _ in 0..BOMB_MB {
                zip.write_all(&chunk).unwrap();
            }

            zip.start_file("readme.txt", options).unwrap();
            zip.write_all(b"honest-entry-marker").unwrap();

            zip.finish().unwrap();
        }

        forge_declared_size(&zip_path, 1024);

        let on_disk = fs::metadata(&zip_path).unwrap().len();
        assert!(
            on_disk < MAX_COMPRESSED_SIZE,
            "fixture must pass the compressed-input cap ({on_disk} bytes)"
        );

        let doc = ArchiveExtractor::new()
            .extract(&zip_path)
            .expect("the honest entry should still extract");

        assert!(
            doc.text.contains("honest-entry-marker"),
            "the honest entry must survive the guard"
        );
        assert!(
            !doc.text.contains(&"A".repeat(4096)),
            "bomb payload reached the output ({} bytes of text)",
            doc.text.len()
        );
        let total: u64 = doc.metadata["total_size"].parse().unwrap();
        assert!(
            total < MAX_SINGLE_FILE_SIZE,
            "accounted {total} bytes — the 45 MB entry was read in full"
        );
    }

    /// TAR had no bound on its compressed input at all.
    #[test]
    fn tar_input_over_compressed_cap_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let tar_path = dir.path().join("big.tar");

        let mut file = File::create(&tar_path).unwrap();
        let chunk = vec![0u8; 1024 * 1024];
        for _ in 0..(MAX_COMPRESSED_SIZE / (1024 * 1024)) + 1 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);

        let err = ArchiveExtractor::new()
            .extract(&tar_path)
            .expect_err("oversized TAR input must be refused")
            .to_string();

        assert!(
            err.contains("Archive too large"),
            "expected the input-size guard, got: {err}"
        );
    }

    /// A `.tar.gz` whose members inflate far past the extraction budget must
    /// stop at the budget, not inflate everything the gzip member contains.
    ///
    /// Before the fix nothing bounded `GzDecoder`: the middle entry's header
    /// declared 150 MB, so the walk skipped it — by streaming and discarding
    /// all 150 MB — and then happily reached the entry after it.
    #[test]
    fn targz_decompressed_stream_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let tgz_path = dir.path().join("bomb.tar.gz");

        {
            let out = File::create(&tgz_path).unwrap();
            let gz = flate2::write::GzEncoder::new(out, flate2::Compression::fast());
            let mut builder = tar::Builder::new(gz);

            let first = b"first-file-marker";
            let mut header = tar::Header::new_gnu();
            header.set_size(first.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "a-first.txt", &first[..])
                .unwrap();

            // 150 MB member — past MAX_EXTRACTED_SIZE on its own.
            const FILLER_MB: u64 = 150;
            let mut header = tar::Header::new_gnu();
            header.set_size(FILLER_MB * 1024 * 1024);
            header.set_mode(0o644);
            header.set_cksum();
            let filler = std::io::repeat(b'A').take(FILLER_MB * 1024 * 1024);
            builder
                .append_data(&mut header, "b-filler.txt", filler)
                .unwrap();

            let last = b"third-file-marker";
            let mut header = tar::Header::new_gnu();
            header.set_size(last.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "c-last.txt", &last[..])
                .unwrap();

            builder.into_inner().unwrap().finish().unwrap();
        }

        let on_disk = fs::metadata(&tgz_path).unwrap().len();
        assert!(
            on_disk < MAX_COMPRESSED_SIZE,
            "fixture must pass the compressed-input cap ({on_disk} bytes)"
        );

        let doc = ArchiveExtractor::new()
            .extract(&tgz_path)
            .expect("entries before the budget should still extract");

        assert!(
            doc.text.contains("first-file-marker"),
            "content before the budget must survive"
        );
        assert!(
            !doc.text.contains("third-file-marker"),
            "the walk ran past the extraction budget to reach the trailing entry"
        );
    }
}
