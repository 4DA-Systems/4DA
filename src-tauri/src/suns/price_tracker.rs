//! Price Tracker Sun -- checks regional data for updates (weekly).

use super::SunResult;

pub fn execute() -> SunResult {
    // Verify that regional data files exist and are readable.
    // Future: opt-in public API fetch for live electricity rates.
    let data_dir = Some(crate::runtime_paths::RuntimePaths::get().streets_regions_dir());

    match data_dir {
        Some(dir) if dir.exists() => {
            let file_count = std::fs::read_dir(&dir)
                .map(|entries| entries.filter(std::result::Result::is_ok).count())
                .unwrap_or(0);

            SunResult {
                success: true,
                message: format!("{file_count} regional data files available"),
                data: Some(serde_json::json!({ "file_count": file_count })),
            }
        }
        _ => SunResult {
            success: true,
            message: "Regional data directory not found (non-fatal)".into(),
            data: Some(serde_json::json!({ "file_count": 0 })),
        },
    }
}
