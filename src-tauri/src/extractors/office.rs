// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/// Office document extraction (Word, Excel)
///
/// Extracts text from Microsoft Office documents:
/// - DOCX (Word) using docx-rs
/// - XLSX (Excel) using calamine
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use crate::error::{Result, ResultExt};
use crate::utils::sanitize_path;
use calamine::{open_workbook, Reader, Xlsx};
use docx_rs::read_docx;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub struct OfficeExtractor;

impl OfficeExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Maximum office document size (100 MB) to prevent OOM on malicious/huge files.
    /// This bounds the **compressed** input only — see `MAX_OFFICE_DECOMPRESSED`.
    const MAX_OFFICE_SIZE: u64 = 100 * 1024 * 1024;

    /// Maximum total decompressed size of an OOXML package (250 MB).
    ///
    /// DOCX/XLSX are ZIP containers. `MAX_OFFICE_SIZE` bounds the file on
    /// disk, which an attacker controls independently of what it expands to:
    /// a 100 MB DOCX of near-zero-entropy XML expands to ~100 GB, and both
    /// `read_docx` and calamine buffer the expansion in memory before we ever
    /// see a parse result. This bounds what actually comes out.
    const MAX_OFFICE_DECOMPRESSED: u64 = 250 * 1024 * 1024;

    /// Maximum decompressed:compressed ratio for an OOXML package.
    /// Real documents sit well under 20:1 (text plus already-compressed media).
    const MAX_OFFICE_RATIO: u64 = 200;

    /// Maximum number of parts in an OOXML package. Real documents have tens.
    const MAX_OFFICE_ENTRIES: usize = 10_000;

    /// Reject OOXML decompression bombs before handing the file to a parser
    /// that buffers the whole expansion.
    ///
    /// Streams every part through `io::sink()` — decompression CPU, no
    /// allocation — and counts the bytes that *actually* come out. Header
    /// fields are not consulted, because they are attacker-controlled.
    fn guard_ooxml_bomb(path: &Path, kind: &str) -> Result<()> {
        let compressed_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if compressed_size > Self::MAX_OFFICE_SIZE {
            return Err(crate::error::FourDaError::Internal(format!(
                "{kind} file too large ({:.1} MB, max {:.0} MB)",
                compressed_size as f64 / (1024.0 * 1024.0),
                Self::MAX_OFFICE_SIZE as f64 / (1024.0 * 1024.0)
            )));
        }

        let file = fs::File::open(path).context("Failed to open Office file")?;
        let mut archive = match zip::ZipArchive::new(io::BufReader::new(file)) {
            Ok(a) => a,
            // Not a readable ZIP — let the real parser produce the error, so
            // the message stays specific to the format.
            Err(_) => return Ok(()),
        };

        if archive.len() > Self::MAX_OFFICE_ENTRIES {
            return Err(crate::error::FourDaError::Internal(format!(
                "{kind} package has {} parts (max {}) — refusing to parse",
                archive.len(),
                Self::MAX_OFFICE_ENTRIES
            )));
        }

        let mut total: u64 = 0;
        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.is_dir() {
                continue;
            }

            // Read one byte past the remaining budget so overshoot is visible.
            let budget = Self::MAX_OFFICE_DECOMPRESSED.saturating_sub(total) + 1;
            let written = io::copy(&mut (&mut entry).take(budget), &mut io::sink())
                .context("Failed to inflate Office package")?;
            total = total.saturating_add(written);

            if total > Self::MAX_OFFICE_DECOMPRESSED {
                return Err(crate::error::FourDaError::Internal(format!(
                    "{kind} decompression bomb: expands past {:.0} MB (compressed {:.1} MB) — refusing to parse",
                    Self::MAX_OFFICE_DECOMPRESSED as f64 / (1024.0 * 1024.0),
                    compressed_size as f64 / (1024.0 * 1024.0)
                )));
            }
        }

        if compressed_size > 0 && total / compressed_size.max(1) > Self::MAX_OFFICE_RATIO {
            return Err(crate::error::FourDaError::Internal(format!(
                "{kind} decompression bomb: {}:1 compression ratio exceeds {}:1 — refusing to parse",
                total / compressed_size.max(1),
                Self::MAX_OFFICE_RATIO
            )));
        }

        Ok(())
    }

    /// Extract text from a DOCX file
    fn extract_docx(&self, path: &Path) -> Result<ExtractedDocument> {
        Self::guard_ooxml_bomb(path, "DOCX")?;
        let bytes = fs::read(path).context("Failed to read DOCX file")?;

        let docx = read_docx(&bytes).context("Failed to parse DOCX structure")?;

        let mut text_parts: Vec<String> = Vec::new();
        let metadata = HashMap::new();

        // Note: Metadata extraction skipped for now due to API complexity
        // Core properties available at docx.doc_props.core but structure varies

        // Extract text from document body
        for child in docx.document.children {
            if let docx_rs::DocumentChild::Paragraph(para) = child {
                let para_text = extract_paragraph_text(&para);
                if !para_text.trim().is_empty() {
                    text_parts.push(para_text);
                }
            } else if let docx_rs::DocumentChild::Table(table) = child {
                // Extract text from tables
                for row in &table.rows {
                    let docx_rs::TableChild::TableRow(tr) = row;
                    let mut row_cells: Vec<String> = Vec::new();
                    for cell in &tr.cells {
                        let docx_rs::TableRowChild::TableCell(tc) = cell;
                        let mut cell_text = String::new();
                        for content in &tc.children {
                            if let docx_rs::TableCellContent::Paragraph(p) = content {
                                cell_text.push_str(&extract_paragraph_text(p));
                                cell_text.push(' ');
                            }
                        }
                        row_cells.push(cell_text.trim().to_string());
                    }
                    if !row_cells.iter().all(std::string::String::is_empty) {
                        text_parts.push(row_cells.join(" | "));
                    }
                }
            }
        }

        let full_text = text_parts.join("\n");

        if full_text.trim().is_empty() {
            return Err("No text content found in DOCX document".into());
        }

        // Create single page for DOCX (no natural page breaks available)
        let pages = vec![PageContent {
            page_number: 1,
            text: full_text.clone(),
            confidence: Some(1.0),
        }];

        Ok(ExtractedDocument {
            text: full_text,
            metadata,
            pages,
            confidence: 1.0,
            source_type: "docx".to_string(),
        })
    }

    /// Extract text from an XLSX file
    fn extract_xlsx(&self, path: &Path) -> Result<ExtractedDocument> {
        Self::guard_ooxml_bomb(path, "XLSX")?;
        let mut workbook: Xlsx<_> = open_workbook(path).context("Failed to open Excel workbook")?;

        let mut all_text: Vec<String> = Vec::new();
        let mut metadata = HashMap::new();
        let mut pages: Vec<PageContent> = Vec::new();

        let sheet_names = workbook.sheet_names().clone();
        metadata.insert("sheet_count".to_string(), sheet_names.len().to_string());

        if sheet_names.len() > 100 {
            tracing::warn!(target: "4da::extractors", total = sheet_names.len(), "XLSX has >100 sheets — processing first 100 only");
        }

        for (idx, sheet_name) in sheet_names.iter().enumerate().take(100) {
            let mut sheet_text: Vec<String> = Vec::new();
            sheet_text.push(format!("=== Sheet: {sheet_name} ==="));

            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                for row in range.rows() {
                    let row_text: Vec<String> = row
                        .iter()
                        .map(cell_to_string)
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !row_text.is_empty() {
                        sheet_text.push(row_text.join(" | "));
                    }
                }
            }

            let sheet_content = sheet_text.join("\n");
            if sheet_content.lines().count() > 1 {
                // Has more than just the header
                pages.push(PageContent {
                    page_number: idx + 1,
                    text: sheet_content.clone(),
                    confidence: Some(1.0),
                });
                all_text.push(sheet_content);
            }
        }

        let full_text = all_text.join("\n\n");

        if full_text.trim().is_empty() || pages.is_empty() {
            return Err("No data found in Excel workbook".into());
        }

        Ok(ExtractedDocument {
            text: full_text,
            metadata,
            pages,
            confidence: 1.0,
            source_type: "xlsx".to_string(),
        })
    }
}

impl Default for OfficeExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for OfficeExtractor {
    fn supported_extensions(&self) -> &[&str] {
        &["docx", "xlsx"]
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
            .map(str::to_lowercase)
            .ok_or_else(|| "File has no extension".to_string())?;

        match ext.as_str() {
            "docx" => self.extract_docx(path),
            "xlsx" => self.extract_xlsx(path),
            _ => Err(format!("Unsupported Office format: {ext}").into()),
        }
    }
}

/// Extract text content from a DOCX paragraph
fn extract_paragraph_text(para: &docx_rs::Paragraph) -> String {
    let mut text = String::new();

    for child in &para.children {
        if let docx_rs::ParagraphChild::Run(run) = child {
            for run_child in &run.children {
                if let docx_rs::RunChild::Text(t) = run_child {
                    text.push_str(&t.text);
                }
            }
        }
    }

    text
}

/// Convert Excel cell to string representation
fn cell_to_string(cell: &calamine::Data) -> String {
    use calamine::Data;

    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            // Format floats nicely (avoid unnecessary decimals)
            if f.fract() == 0.0 {
                format!("{f:.0}")
            } else {
                format!("{f:.2}")
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::Error(e) => format!("#ERR:{e:?}"),
        Data::DateTime(dt) => format!("{dt}"),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_office_supported_extensions() {
        let extractor = OfficeExtractor::new();
        let exts = extractor.supported_extensions();
        assert!(exts.contains(&"docx"));
        assert!(exts.contains(&"xlsx"));
        // Legacy formats not supported yet
        assert!(!exts.contains(&"doc"));
        assert!(!exts.contains(&"xls"));
    }

    #[test]
    fn test_office_can_handle() {
        let extractor = OfficeExtractor::new();
        assert!(extractor.can_handle(Path::new("test.docx")));
        assert!(extractor.can_handle(Path::new("test.XLSX")));
        assert!(!extractor.can_handle(Path::new("test.pdf")));
        assert!(!extractor.can_handle(Path::new("test.doc"))); // Legacy not supported
    }

    #[test]
    fn test_office_nonexistent_file() {
        let extractor = OfficeExtractor::new();
        let result = extractor.extract(Path::new("/nonexistent/file.docx"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_cell_to_string_int() {
        let cell = calamine::Data::Int(42);
        assert_eq!(cell_to_string(&cell), "42");
    }

    #[test]
    fn test_cell_to_string_float() {
        let cell = calamine::Data::Float(std::f64::consts::PI);
        assert_eq!(cell_to_string(&cell), "3.14");
    }

    #[test]
    fn test_cell_to_string_whole_number_float() {
        let cell = calamine::Data::Float(100.0);
        assert_eq!(cell_to_string(&cell), "100");
    }

    #[test]
    fn test_cell_to_string_bool() {
        assert_eq!(cell_to_string(&calamine::Data::Bool(true)), "TRUE");
        assert_eq!(cell_to_string(&calamine::Data::Bool(false)), "FALSE");
    }

    #[test]
    fn test_cell_to_string_empty() {
        let cell = calamine::Data::Empty;
        assert_eq!(cell_to_string(&cell), "");
    }

    #[test]
    fn test_cell_to_string_text() {
        let cell = calamine::Data::String("Hello World".to_string());
        assert_eq!(cell_to_string(&cell), "Hello World");
    }

    // ================================================================
    // Decompression-bomb guards
    //
    // These replace two `#[ignore]`d tests that were also self-neutering:
    // each wrapped its only assertion in `if test_docx.exists()` against a
    // temp path nothing ever created, so even when un-ignored they asserted
    // nothing. Fixtures below are built programmatically at test time — no
    // binaries are committed.
    // ================================================================

    use std::io::Write as _;

    /// Build a single-entry OOXML package whose one part inflates to
    /// `payload_mb` MB of a single repeated byte, at the given deflate level.
    ///
    /// The level matters: at level 1 a run of `b'A'` lands near 200:1, which
    /// is exactly `MAX_OFFICE_RATIO` — a fixture sitting on its own threshold
    /// flips with any flate2 bump. Callers that mean to exercise the ratio
    /// guard pass level 9 (~1000:1) and assert the margin.
    fn write_bomb(path: &Path, payload_mb: usize, level: i32) {
        let file = fs::File::create(path).expect("create bomb fixture");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(level));

        zip.start_file("word/document.xml", options)
            .expect("start entry");
        let chunk = vec![b'A'; 1024 * 1024];
        for _ in 0..payload_mb {
            zip.write_all(&chunk).expect("write bomb payload");
        }
        zip.finish().expect("finish bomb fixture");
    }

    /// The headline case: a small file on disk that expands past the
    /// decompressed cap. `MAX_OFFICE_SIZE` (compressed) never sees it.
    #[test]
    fn docx_decompression_bomb_is_refused_on_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bomb = dir.path().join("bomb.docx");
        write_bomb(&bomb, 300, 1);

        let on_disk = fs::metadata(&bomb).expect("stat bomb").len();
        assert!(
            on_disk < OfficeExtractor::MAX_OFFICE_SIZE,
            "fixture must pass the compressed-size cap ({on_disk} bytes) — otherwise it proves nothing about the decompressed bound"
        );

        let err = OfficeExtractor::new()
            .extract(&bomb)
            .expect_err("300 MB expansion must be refused")
            .to_string();

        assert!(
            err.contains("decompression bomb") && err.contains("expands past"),
            "expected the decompressed-size guard, got: {err}"
        );
    }

    /// The ratio guard, on a payload small enough not to trip the size cap.
    #[test]
    fn docx_decompression_bomb_is_refused_on_ratio() {
        const PAYLOAD_MB: u64 = 40;
        let dir = tempfile::tempdir().expect("tempdir");
        let bomb = dir.path().join("ratio.docx");
        write_bomb(&bomb, PAYLOAD_MB as usize, 9);

        // Guard the guard: if a flate2 bump ever drags this fixture's ratio
        // back toward the threshold, fail here with the reason rather than
        // downstream with a confusing parser error.
        let on_disk = fs::metadata(&bomb).expect("stat bomb").len();
        let ratio = (PAYLOAD_MB * 1024 * 1024) / on_disk.max(1);
        assert!(
            ratio > OfficeExtractor::MAX_OFFICE_RATIO * 2,
            "fixture is no longer a convincing bomb: {ratio}:1 against a {}:1 threshold",
            OfficeExtractor::MAX_OFFICE_RATIO
        );
        assert!(
            PAYLOAD_MB * 1024 * 1024 < OfficeExtractor::MAX_OFFICE_DECOMPRESSED,
            "fixture must stay under the absolute size cap so the ratio guard is what fires"
        );

        let err = match OfficeExtractor::new().extract(&bomb) {
            Ok(doc) => panic!(
                "a {ratio}:1 package must be refused, got {} chars of text",
                doc.text.len()
            ),
            Err(e) => e.to_string(),
        };

        assert!(
            err.contains("decompression bomb") && err.contains("compression ratio"),
            "expected the ratio guard, got: {err}"
        );
    }

    /// XLSX runs through the same guard — calamine buffers the expansion too.
    #[test]
    fn xlsx_decompression_bomb_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bomb = dir.path().join("bomb.xlsx");
        write_bomb(&bomb, 300, 1);

        let err = OfficeExtractor::new()
            .extract(&bomb)
            .expect_err("300 MB expansion must be refused")
            .to_string();

        assert!(
            err.contains("XLSX decompression bomb"),
            "expected the XLSX guard, got: {err}"
        );
    }

    /// A package with an absurd part count is refused before any inflation.
    #[test]
    fn ooxml_entry_count_bomb_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bomb = dir.path().join("many.docx");

        let file = fs::File::create(&bomb).expect("create fixture");
        let mut zip = zip::ZipWriter::new(file);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for i in 0..=OfficeExtractor::MAX_OFFICE_ENTRIES {
            zip.start_file(format!("part{i}.xml"), options)
                .expect("start entry");
        }
        zip.finish().expect("finish fixture");

        let err = OfficeExtractor::new()
            .extract(&bomb)
            .expect_err("entry-count bomb must be refused")
            .to_string();

        assert!(
            err.contains("parts (max"),
            "expected the entry-count guard, got: {err}"
        );
    }

    /// Anti-false-positive: a genuine DOCX, built with the same library that
    /// reads it, still extracts. Guards that reject everything are not guards.
    #[test]
    fn legitimate_docx_still_extracts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let doc_path = dir.path().join("real.docx");

        let file = fs::File::create(&doc_path).expect("create docx");
        docx_rs::Docx::new()
            .add_paragraph(
                docx_rs::Paragraph::new()
                    .add_run(docx_rs::Run::new().add_text("Quarterly revenue summary")),
            )
            .add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Second paragraph")),
            )
            .build()
            .pack(file)
            .expect("pack docx");

        let doc = OfficeExtractor::new()
            .extract(&doc_path)
            .expect("a real DOCX must still extract");

        assert_eq!(doc.source_type, "docx");
        assert!(
            doc.text.contains("Quarterly revenue summary"),
            "missing first paragraph: {}",
            doc.text
        );
        assert!(
            doc.text.contains("Second paragraph"),
            "missing second paragraph: {}",
            doc.text
        );
    }
}
