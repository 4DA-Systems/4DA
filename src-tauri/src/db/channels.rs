//! Channel database operations — CRUD, renders, provenance, source matching.

use rusqlite::{params, Result as SqliteResult};
use tracing::info;

use crate::channels::{
    Channel, ChannelFreshness, ChannelRender, ChannelSourceMatch, ChannelStatus, ChannelSummary,
    RenderProvenance, SEED_CHANNELS,
};

use super::Database;

impl Database {
    // ========================================================================
    // Channel CRUD
    // ========================================================================

    /// Create a new channel with the given metadata.
    pub fn create_channel(
        &self,
        slug: &str,
        title: &str,
        description: &str,
        topic_query: &[String],
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let topics_json = serde_json::to_string(topic_query).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO channels (slug, title, description, topic_query, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', datetime('now'), datetime('now'))",
            params![slug, title, description, topics_json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch a channel by its primary key.
    pub fn get_channel(&self, channel_id: i64) -> SqliteResult<Channel> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, slug, title, description, topic_query, status,
                    source_count, render_count, last_rendered_at, created_at, updated_at
             FROM channels WHERE id = ?1",
            params![channel_id],
            |row| {
                let topics_json: String = row.get(4)?;
                let status_str: String = row.get(5)?;
                Ok(Channel {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    topic_query: serde_json::from_str(&topics_json).unwrap_or_default(),
                    status: match status_str.as_str() {
                        "paused" => ChannelStatus::Paused,
                        "archived" => ChannelStatus::Archived,
                        _ => ChannelStatus::Active,
                    },
                    source_count: row.get(6)?,
                    render_count: row.get(7)?,
                    last_rendered_at: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
    }

    /// Fetch a channel by its unique slug.
    pub fn get_channel_by_slug(&self, slug: &str) -> SqliteResult<Channel> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, slug, title, description, topic_query, status,
                    source_count, render_count, last_rendered_at, created_at, updated_at
             FROM channels WHERE slug = ?1",
            params![slug],
            |row| {
                let topics_json: String = row.get(4)?;
                let status_str: String = row.get(5)?;
                Ok(Channel {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    topic_query: serde_json::from_str(&topics_json).unwrap_or_default(),
                    status: match status_str.as_str() {
                        "paused" => ChannelStatus::Paused,
                        "archived" => ChannelStatus::Archived,
                        _ => ChannelStatus::Active,
                    },
                    source_count: row.get(6)?,
                    render_count: row.get(7)?,
                    last_rendered_at: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
    }

    /// List all active channels as lightweight summaries.
    pub fn list_channels(&self) -> SqliteResult<Vec<ChannelSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, slug, title, description, source_count, render_count, last_rendered_at
             FROM channels WHERE status = 'active' ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            let last_rendered: Option<String> = row.get(6)?;
            let freshness = match &last_rendered {
                None => ChannelFreshness::NeverRendered,
                Some(ts) => {
                    let rendered = super::parse_datetime(ts.clone());
                    let age = chrono::Utc::now() - rendered;
                    if age.num_hours() < 24 {
                        ChannelFreshness::Fresh
                    } else {
                        ChannelFreshness::Stale
                    }
                }
            };
            Ok(ChannelSummary {
                id: row.get(0)?,
                slug: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                source_count: row.get(4)?,
                render_count: row.get(5)?,
                freshness,
                last_rendered_at: last_rendered,
            })
        })?;
        rows.collect()
    }

    /// Update a channel's lifecycle status (active / paused / archived).
    pub fn update_channel_status(
        &self,
        channel_id: i64,
        status: &ChannelStatus,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let status_str = match status {
            ChannelStatus::Active => "active",
            ChannelStatus::Paused => "paused",
            ChannelStatus::Archived => "archived",
        };
        conn.execute(
            "UPDATE channels SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status_str, channel_id],
        )?;
        Ok(())
    }

    /// Count active custom channels (excludes seed/default channels).
    pub fn count_custom_channels(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let seed_slugs: Vec<&str> = SEED_CHANNELS.iter().map(|s| s.slug).collect();
        let placeholders: String = seed_slugs
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT COUNT(*) FROM channels WHERE status = 'active' AND slug NOT IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = seed_slugs
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();
        stmt.query_row(params.as_slice(), |row| row.get(0))
    }

    // ========================================================================
    // Source Matching
    // ========================================================================

    /// Insert or update a channel-source match score.
    pub fn upsert_channel_source_match(
        &self,
        channel_id: i64,
        source_item_id: i64,
        match_score: f64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO channel_source_matches (channel_id, source_item_id, match_score, matched_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(channel_id, source_item_id)
             DO UPDATE SET match_score = ?3, matched_at = datetime('now')",
            params![channel_id, source_item_id, match_score],
        )?;
        Ok(())
    }

    /// Fetch matched source items for a channel, ordered by score descending.
    pub fn get_channel_source_items(
        &self,
        channel_id: i64,
        limit: usize,
    ) -> SqliteResult<Vec<ChannelSourceMatch>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT csm.channel_id, csm.source_item_id, si.title, si.url,
                    si.source_type, csm.match_score, csm.matched_at
             FROM channel_source_matches csm
             JOIN source_items si ON si.id = csm.source_item_id
             WHERE csm.channel_id = ?1
             ORDER BY csm.match_score DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![channel_id, limit as i64], |row| {
            Ok(ChannelSourceMatch {
                channel_id: row.get(0)?,
                source_item_id: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                source_type: row.get(4)?,
                match_score: row.get(5)?,
                matched_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Recount matched sources and update the channel's `source_count`.
    pub fn refresh_channel_source_count(&self, channel_id: i64) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM channel_source_matches WHERE channel_id = ?1",
            params![channel_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "UPDATE channels SET source_count = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![count, channel_id],
        )?;
        Ok(count)
    }

    // ========================================================================
    // Render Snapshots
    // ========================================================================

    /// Save a new render snapshot, auto-incrementing the version number.
    pub fn save_channel_render(
        &self,
        channel_id: i64,
        content_markdown: &str,
        source_item_ids: &[i64],
        model: Option<&str>,
        tokens_used: Option<i64>,
        latency_ms: Option<i64>,
    ) -> SqliteResult<ChannelRender> {
        let conn = self.conn.lock();
        let content_hash = super::hash_content(content_markdown);
        let ids_json = serde_json::to_string(source_item_ids).unwrap_or_else(|_| "[]".to_string());

        // Auto-increment version within the channel
        let next_version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM channel_renders WHERE channel_id = ?1",
            params![channel_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO channel_renders
                (channel_id, version, content_markdown, content_hash,
                 source_item_ids, model, tokens_used, latency_ms, rendered_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))",
            params![
                channel_id,
                next_version,
                content_markdown,
                content_hash,
                ids_json,
                model,
                tokens_used,
                latency_ms
            ],
        )?;
        let render_id = conn.last_insert_rowid();

        // Update channel metadata
        conn.execute(
            "UPDATE channels SET last_rendered_at = datetime('now'),
                    render_count = render_count + 1, updated_at = datetime('now')
             WHERE id = ?1",
            params![channel_id],
        )?;

        Ok(ChannelRender {
            id: render_id,
            channel_id,
            version: next_version,
            content_markdown: content_markdown.to_string(),
            content_hash,
            source_item_ids: source_item_ids.to_vec(),
            model: model.map(std::string::ToString::to_string),
            tokens_used,
            latency_ms,
            rendered_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    /// Get the most recent render for a channel, if any.
    pub fn get_latest_render(&self, channel_id: i64) -> SqliteResult<Option<ChannelRender>> {
        let conn = self.conn.lock();
        let result = conn.query_row(
            "SELECT id, channel_id, version, content_markdown, content_hash,
                    source_item_ids, model, tokens_used, latency_ms, rendered_at
             FROM channel_renders
             WHERE channel_id = ?1
             ORDER BY version DESC LIMIT 1",
            params![channel_id],
            |row| {
                let ids_json: String = row.get(5)?;
                Ok(ChannelRender {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    version: row.get(2)?,
                    content_markdown: row.get(3)?,
                    content_hash: row.get(4)?,
                    source_item_ids: serde_json::from_str(&ids_json).unwrap_or_default(),
                    model: row.get(6)?,
                    tokens_used: row.get(7)?,
                    latency_ms: row.get(8)?,
                    rendered_at: row.get(9)?,
                })
            },
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the N most recent renders for a channel, newest first.
    pub fn get_render_history(
        &self,
        channel_id: i64,
        limit: usize,
    ) -> SqliteResult<Vec<ChannelRender>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, version, content_markdown, content_hash,
                    source_item_ids, model, tokens_used, latency_ms, rendered_at
             FROM channel_renders
             WHERE channel_id = ?1
             ORDER BY version DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![channel_id, limit as i64], |row| {
            let ids_json: String = row.get(5)?;
            Ok(ChannelRender {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                version: row.get(2)?,
                content_markdown: row.get(3)?,
                content_hash: row.get(4)?,
                source_item_ids: serde_json::from_str(&ids_json).unwrap_or_default(),
                model: row.get(6)?,
                tokens_used: row.get(7)?,
                latency_ms: row.get(8)?,
                rendered_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    // ========================================================================
    // Provenance
    // ========================================================================

    /// Save provenance records linking render claims to source items.
    /// Uses a transaction to batch all inserts for performance.
    pub fn save_render_provenance(&self, provenance: &[RenderProvenance]) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO channel_provenance
                    (render_id, claim_index, claim_text, source_item_ids, source_titles, source_urls)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for p in provenance {
                let ids_json =
                    serde_json::to_string(&p.source_item_ids).unwrap_or_else(|_| "[]".to_string());
                let titles_json =
                    serde_json::to_string(&p.source_titles).unwrap_or_else(|_| "[]".to_string());
                let urls_json =
                    serde_json::to_string(&p.source_urls).unwrap_or_else(|_| "[]".to_string());
                stmt.execute(params![
                    p.render_id,
                    p.claim_index,
                    p.claim_text,
                    ids_json,
                    titles_json,
                    urls_json
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Load all provenance records for a render, ordered by claim index.
    pub fn get_render_provenance(&self, render_id: i64) -> SqliteResult<Vec<RenderProvenance>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT render_id, claim_index, claim_text,
                    source_item_ids, source_titles, source_urls
             FROM channel_provenance
             WHERE render_id = ?1
             ORDER BY claim_index",
        )?;
        let rows = stmt.query_map(params![render_id], |row| {
            let ids_json: String = row.get(3)?;
            let titles_json: String = row.get(4)?;
            let urls_json: String = row.get(5)?;
            Ok(RenderProvenance {
                render_id: row.get(0)?,
                claim_index: row.get(1)?,
                claim_text: row.get(2)?,
                source_item_ids: serde_json::from_str(&ids_json).unwrap_or_default(),
                source_titles: serde_json::from_str(&titles_json).unwrap_or_default(),
                source_urls: serde_json::from_str(&urls_json).unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    // ========================================================================
    // Seed
    // ========================================================================

    /// Insert a channel only if its slug doesn't already exist (idempotent).
    pub fn seed_channel_if_absent(
        &self,
        slug: &str,
        title: &str,
        description: &str,
        topics: &[&str],
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM channels WHERE slug = ?1",
            params![slug],
            |row| row.get::<_, i64>(0).map(|c| c > 0),
        )?;
        if !exists {
            let topics_json = serde_json::to_string(&topics).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "INSERT INTO channels
                    (slug, title, description, topic_query, status,
                     source_count, render_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'active', 0, 0, datetime('now'), datetime('now'))",
                params![slug, title, description, topics_json],
            )?;
            info!(target: "4da::channels", slug = slug, "Seeded channel");
        }
        Ok(())
    }

    /// Seed all default channels from the static SEED_CHANNELS list.
    pub fn seed_default_channels(&self) -> SqliteResult<()> {
        for seed in SEED_CHANNELS {
            self.seed_channel_if_absent(seed.slug, seed.title, seed.description, seed.topics)?;
        }
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::test_utils::{seed_embedding, test_db};

    #[test]
    fn test_channel_crud() {
        let db = test_db();
        let id = db
            .create_channel(
                "test-channel",
                "Test Channel",
                "A test",
                &["rust".to_string(), "wasm".to_string()],
            )
            .unwrap();
        assert!(id > 0);

        let ch = db.get_channel(id).unwrap();
        assert_eq!(ch.slug, "test-channel");
        assert_eq!(ch.topic_query, vec!["rust", "wasm"]);

        let ch2 = db.get_channel_by_slug("test-channel").unwrap();
        assert_eq!(ch2.id, id);
    }

    #[test]
    fn test_channel_list() {
        let db = test_db();
        // Migrations seed 3 default channels
        let baseline = db.list_channels().unwrap().len();
        db.create_channel("ch-1", "Channel 1", "First", &["a".to_string()])
            .unwrap();
        db.create_channel("ch-2", "Channel 2", "Second", &["b".to_string()])
            .unwrap();
        let list = db.list_channels().unwrap();
        assert_eq!(list.len(), baseline + 2);
    }

    #[test]
    fn test_render_save_and_load() {
        let db = test_db();
        let ch_id = db
            .create_channel("render-test", "Render Test", "test", &[])
            .unwrap();

        let render = db
            .save_channel_render(
                ch_id,
                "# Hello\nWorld",
                &[1, 2, 3],
                Some("llama3"),
                Some(500),
                Some(1200),
            )
            .unwrap();
        assert_eq!(render.version, 1);

        let render2 = db
            .save_channel_render(
                ch_id,
                "# Updated\nContent",
                &[4, 5],
                Some("llama3"),
                None,
                None,
            )
            .unwrap();
        assert_eq!(render2.version, 2);

        let latest = db.get_latest_render(ch_id).unwrap().unwrap();
        assert_eq!(latest.version, 2);
        assert!(latest.content_markdown.contains("Updated"));
    }

    #[test]
    fn test_provenance() {
        let db = test_db();
        let ch_id = db
            .create_channel("prov-test", "Prov Test", "test", &[])
            .unwrap();
        let render = db
            .save_channel_render(ch_id, "Content", &[], None, None, None)
            .unwrap();

        let prov = vec![crate::channels::RenderProvenance {
            render_id: render.id,
            claim_index: 0,
            claim_text: "Test claim".to_string(),
            source_item_ids: vec![1, 2],
            source_titles: vec!["Title 1".to_string(), "Title 2".to_string()],
            source_urls: vec![
                "https://example.com/1".to_string(),
                "https://example.com/2".to_string(),
            ],
        }];
        db.save_render_provenance(&prov).unwrap();

        let loaded = db.get_render_provenance(render.id).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].source_item_ids, vec![1, 2]);
    }

    #[test]
    fn test_seed_channels() {
        let db = test_db();
        db.seed_default_channels().unwrap();
        let list = db.list_channels().unwrap();
        assert_eq!(list.len(), 3);
        // Second call should be idempotent
        db.seed_default_channels().unwrap();
        let list2 = db.list_channels().unwrap();
        assert_eq!(list2.len(), 3);
    }

    #[test]
    fn test_source_matches() {
        let db = test_db();
        let ch_id = db
            .create_channel("match-test", "Match Test", "test", &[])
            .unwrap();
        // Create a real source_item to satisfy FK constraint
        let emb = seed_embedding("match-test");
        let item_id = db
            .upsert_source_item("test", "match-item-1", None, "Test Item", "content", &emb)
            .unwrap();
        db.upsert_channel_source_match(ch_id, item_id, 0.85)
            .unwrap();
        let count = db.refresh_channel_source_count(ch_id).unwrap();
        assert_eq!(count, 1);
    }
}
