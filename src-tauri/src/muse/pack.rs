//! MUSE Context Pack Lifecycle
//!
//! Create, update, blend, activate, and export context packs.
//! Phase 0: core operations with database persistence.
//! Phase 1+: extractors populate profiles during pack creation.

use rusqlite::{params, Connection};
use tracing::info;
use uuid::Uuid;

use crate::error::{Result, ResultExt};

use super::{MuseMediaType, MusePack, MusePackSource, MusePackType, WeightedTopic};

// ============================================================================
// Pack Operations
// ============================================================================

/// Create a new empty context pack
pub fn create_pack(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
) -> Result<MusePack> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        r"INSERT INTO muse_packs (id, name, description, pack_type, is_active, source_count, confidence)
          VALUES (?1, ?2, ?3, 'custom', 0, 0, 0.0)",
        params![id, name, description],
    )
    .context("Failed to create MUSE pack")?;

    info!(target: "muse::pack", pack_id = %id, name = %name, "Created new context pack");

    Ok(MusePack {
        id,
        name: name.to_string(),
        description: description.map(String::from),
        pack_type: MusePackType::Custom,
        is_active: false,
        source_count: 0,
        confidence: 0.0,
        visual: None,
        sonic: None,
        motion: None,
        topics: Vec::new(),
        anti_patterns: Vec::new(),
        style_centroid: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// List all packs, optionally filtered by active status
pub fn list_packs(conn: &Connection, active_only: bool) -> Result<Vec<MusePack>> {
    let query = if active_only {
        "SELECT id, name, description, pack_type, is_active, source_count, confidence,
                thematic_topics, anti_patterns, style_embedding, created_at, updated_at
         FROM muse_packs WHERE is_active = 1 ORDER BY updated_at DESC"
    } else {
        "SELECT id, name, description, pack_type, is_active, source_count, confidence,
                thematic_topics, anti_patterns, style_embedding, created_at, updated_at
         FROM muse_packs ORDER BY updated_at DESC"
    };

    let mut stmt = conn.prepare(query).context("Failed to prepare pack list query")?;

    let packs = stmt
        .query_map([], |row| {
            let topics_json: Option<String> = row.get(7)?;
            let anti_json: Option<String> = row.get(8)?;
            let centroid_blob: Option<Vec<u8>> = row.get(9)?;

            let topics: Vec<WeightedTopic> = topics_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();

            let anti_patterns: Vec<WeightedTopic> = anti_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();

            let style_centroid = centroid_blob.map(|blob| base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &blob,
            ));

            let pack_type_str: String = row.get(3)?;
            let pack_type = match pack_type_str.as_str() {
                "auto" => MusePackType::Auto,
                "imported" => MusePackType::Imported,
                "marketplace" => MusePackType::Marketplace,
                "blend" => MusePackType::Blend,
                _ => MusePackType::Custom,
            };

            Ok(MusePack {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                pack_type,
                is_active: row.get::<_, i64>(4)? != 0,
                source_count: row.get::<_, u32>(5)?,
                confidence: row.get(6)?,
                visual: None,   // Loaded separately when needed
                sonic: None,
                motion: None,
                topics,
                anti_patterns,
                style_centroid,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .context("Failed to query packs")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(packs)
}

/// Get a single pack by ID
pub fn get_pack(conn: &Connection, pack_id: &str) -> Result<Option<MusePack>> {
    let packs = list_packs(conn, false)?;
    Ok(packs.into_iter().find(|p| p.id == pack_id))
}

/// Activate or deactivate a pack
pub fn set_pack_active(conn: &Connection, pack_id: &str, active: bool) -> Result<()> {
    conn.execute(
        "UPDATE muse_packs SET is_active = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![active as i64, pack_id],
    )
    .context("Failed to update pack active status")?;

    info!(target: "muse::pack", pack_id = %pack_id, active = %active, "Pack active status updated");
    Ok(())
}

/// Delete a pack and all its sources (CASCADE)
pub fn delete_pack(conn: &Connection, pack_id: &str) -> Result<()> {
    // Delete from vec table first (no CASCADE on virtual tables)
    conn.execute(
        "DELETE FROM muse_style_vec WHERE rowid IN (SELECT rowid FROM muse_packs WHERE id = ?1)",
        [pack_id],
    )
    .ok(); // Best effort — vec entry may not exist

    conn.execute("DELETE FROM muse_packs WHERE id = ?1", [pack_id])
        .context("Failed to delete pack")?;

    info!(target: "muse::pack", pack_id = %pack_id, "Deleted context pack");
    Ok(())
}

/// Add a source file to a pack (queued for extraction)
pub fn add_source(
    conn: &Connection,
    pack_id: &str,
    file_path: &str,
    file_type: MuseMediaType,
    file_hash: Option<&str>,
) -> Result<i64> {
    conn.execute(
        r"INSERT INTO muse_pack_sources (pack_id, file_path, file_type, extraction_status, file_hash)
          VALUES (?1, ?2, ?3, 'pending', ?4)",
        params![pack_id, file_path, file_type.to_string(), file_hash],
    )
    .context("Failed to add source to pack")?;

    let source_id = conn.last_insert_rowid();

    // Update source count on the pack
    conn.execute(
        "UPDATE muse_packs SET source_count = (SELECT COUNT(*) FROM muse_pack_sources WHERE pack_id = ?1), updated_at = datetime('now') WHERE id = ?1",
        [pack_id],
    )
    .context("Failed to update pack source count")?;

    info!(target: "muse::pack", pack_id = %pack_id, file_path = %file_path, "Added source to pack");
    Ok(source_id)
}

/// List sources for a pack
pub fn list_sources(conn: &Connection, pack_id: &str) -> Result<Vec<MusePackSource>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pack_id, file_path, file_type, extraction_status, confidence, file_hash
             FROM muse_pack_sources WHERE pack_id = ?1 ORDER BY created_at",
        )
        .context("Failed to prepare sources query")?;

    let sources = stmt
        .query_map([pack_id], |row| {
            let file_type_str: String = row.get(3)?;
            let status_str: String = row.get(4)?;

            let file_type = match file_type_str.as_str() {
                "video" => MuseMediaType::Video,
                "audio" => MuseMediaType::Audio,
                "document" => MuseMediaType::Document,
                "project_file" => MuseMediaType::ProjectFile,
                _ => MuseMediaType::Image,
            };

            let extraction_status = match status_str.as_str() {
                "processing" => super::ExtractionStatus::Processing,
                "done" => super::ExtractionStatus::Done,
                "failed" => super::ExtractionStatus::Failed,
                _ => super::ExtractionStatus::Pending,
            };

            Ok(MusePackSource {
                id: row.get(0)?,
                pack_id: row.get(1)?,
                file_path: row.get(2)?,
                file_type,
                extraction_status,
                confidence: row.get(5)?,
                file_hash: row.get(6)?,
            })
        })
        .context("Failed to query sources")?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sources)
}

/// Get count of packs by status
pub fn pack_stats(conn: &Connection) -> Result<PackStats> {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM muse_packs", [], |row| row.get(0))
        .unwrap_or(0);

    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM muse_packs WHERE is_active = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_sources: i64 = conn
        .query_row("SELECT COUNT(*) FROM muse_pack_sources", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let total_generations: i64 = conn
        .query_row("SELECT COUNT(*) FROM muse_generations", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    Ok(PackStats {
        total_packs: total as u32,
        active_packs: active as u32,
        total_sources: total_sources as u32,
        total_generations: total_generations as u32,
    })
}

/// Summary statistics for MUSE packs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PackStats {
    pub total_packs: u32,
    pub active_packs: u32,
    pub total_sources: u32,
    pub total_generations: u32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().expect("Failed to open in-memory DB");
        // Run migration via Arc<Mutex<>> (migration API requirement)
        let arc = Arc::new(Mutex::new(conn));
        crate::muse::db::migrate(&arc).expect("MUSE migration failed");
        Arc::try_unwrap(arc).expect("Arc still shared").into_inner()
    }

    #[test]
    fn test_create_and_list_packs() {
        let conn = setup_test_db();

        let pack = create_pack(&conn, "Test Pack", Some("A test")).unwrap();
        assert_eq!(pack.name, "Test Pack");
        assert!(!pack.is_active);

        let packs = list_packs(&conn, false).unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "Test Pack");
    }

    #[test]
    fn test_activate_deactivate_pack() {
        let conn = setup_test_db();

        let pack = create_pack(&conn, "Active Test", None).unwrap();
        set_pack_active(&conn, &pack.id, true).unwrap();

        let active_packs = list_packs(&conn, true).unwrap();
        assert_eq!(active_packs.len(), 1);

        set_pack_active(&conn, &pack.id, false).unwrap();
        let active_packs = list_packs(&conn, true).unwrap();
        assert_eq!(active_packs.len(), 0);
    }

    #[test]
    fn test_add_and_list_sources() {
        let conn = setup_test_db();

        let pack = create_pack(&conn, "Source Test", None).unwrap();
        add_source(&conn, &pack.id, "/art/logo.png", MuseMediaType::Image, None).unwrap();
        add_source(&conn, &pack.id, "/art/track.wav", MuseMediaType::Audio, None).unwrap();

        let sources = list_sources(&conn, &pack.id).unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].file_type, MuseMediaType::Image);
        assert_eq!(sources[1].file_type, MuseMediaType::Audio);

        // Pack source count should be updated
        let pack = get_pack(&conn, &pack.id).unwrap().unwrap();
        assert_eq!(pack.source_count, 2);
    }

    #[test]
    fn test_delete_pack_cascades() {
        let conn = setup_test_db();

        let pack = create_pack(&conn, "Delete Test", None).unwrap();
        add_source(&conn, &pack.id, "/test.png", MuseMediaType::Image, None).unwrap();

        delete_pack(&conn, &pack.id).unwrap();

        let packs = list_packs(&conn, false).unwrap();
        assert!(packs.is_empty());

        // Sources should be cascaded
        let sources = list_sources(&conn, &pack.id).unwrap();
        assert!(sources.is_empty());
    }

    #[test]
    fn test_pack_stats() {
        let conn = setup_test_db();

        create_pack(&conn, "Pack 1", None).unwrap();
        let pack2 = create_pack(&conn, "Pack 2", None).unwrap();
        set_pack_active(&conn, &pack2.id, true).unwrap();
        add_source(&conn, &pack2.id, "/img.png", MuseMediaType::Image, None).unwrap();

        let stats = pack_stats(&conn).unwrap();
        assert_eq!(stats.total_packs, 2);
        assert_eq!(stats.active_packs, 1);
        assert_eq!(stats.total_sources, 1);
        assert_eq!(stats.total_generations, 0);
    }
}
