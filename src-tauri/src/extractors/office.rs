/// Office document extraction (Word, Excel)
///
/// Extracts text from Microsoft Office documents:
/// - DOCX (Word) using docx-rs
/// - XLSX (Excel) using calamine
use super::{DocumentExtractor, ExtractedDocument, PageContent};
use calamine::{open_workbook, Reader, Xlsx};
use docx_rs::read_docx;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct OfficeExtractor;

impl OfficeExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract text from a DOCX file
    fn extract_docx(&self, path: &Path) -> Result<ExtractedDocument, String> {
        let bytes = fs::read(path).map_err(|e| format!("Failed to read DOCX file: {}", e))?;

        let docx =
            read_docx(&bytes).map_err(|e| format!("Failed to parse DOCX structure: {}", e))?;

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
                    if !row_cells.iter().all(|c| c.is_empty()) {
                        text_parts.push(row_cells.join(" | "));
                    }
                }
            }
        }

        let full_text = text_parts.join("\n");

        if full_text.trim().is_empty() {
            return Err("No text content found in DOCX document".to_string());
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
    fn extract_xlsx(&self, path: &Path) -> Result<ExtractedDocument, String> {
        let mut workbook: Xlsx<_> =
            open_workbook(path).map_err(|e| format!("Failed to open Excel workbook: {}", e))?;

        let mut all_text: Vec<String> = Vec::new();
        let mut metadata = HashMap::new();
        let mut pages: Vec<PageContent> = Vec::new();

        let sheet_names = workbook.sheet_names().to_vec();
        metadata.insert("sheet_count".to_string(), sheet_names.len().to_string());

        for (idx, sheet_name) in sheet_names.iter().enumerate() {
            let mut sheet_text: Vec<String> = Vec::new();
            sheet_text.push(format!("=== Sheet: {} ===", sheet_name));

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
            return Err("No data found in Excel workbook".to_string());
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

    fn extract(&self, path: &Path) -> Result<ExtractedDocument, String> {
        if !path.exists() {
            return Err(format!("File does not exist: {:?}", path));
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .ok_or_else(|| "File has no extension".to_string())?;

        match ext.as_str() {
            "docx" => self.extract_docx(path),
            "xlsx" => self.extract_xlsx(path),
            _ => Err(format!("Unsupported Office format: {}", ext)),
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
                format!("{:.0}", f)
            } else {
                format!("{:.2}", f)
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Data::Error(e) => format!("#ERR:{:?}", e),
        Data::DateTime(dt) => format!("{}", dt),
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
        assert!(result.unwrap_err().contains("does not exist"));
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

    // Integration test - requires real Office files
    #[test]
    #[ignore]
    fn test_real_docx_extraction() {
        let test_dir = std::env::temp_dir().join("4da_office_test");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_docx = test_dir.join("test.docx");

        if test_docx.exists() {
            let extractor = OfficeExtractor::new();
            let result = extractor.extract(&test_docx);
            assert!(result.is_ok(), "Should extract DOCX: {:?}", result.err());
        }
    }

    #[test]
    #[ignore]
    fn test_real_xlsx_extraction() {
        let test_dir = std::env::temp_dir().join("4da_office_test");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_xlsx = test_dir.join("test.xlsx");

        if test_xlsx.exists() {
            let extractor = OfficeExtractor::new();
            let result = extractor.extract(&test_xlsx);
            assert!(result.is_ok(), "Should extract XLSX: {:?}", result.err());
        }
    }
}
