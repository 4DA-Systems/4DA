//! Database stress tests — bulk insert, concurrent access, idempotency,
//! embedding roundtrip, and KNN search accuracy.
//!
//! Split from db/mod.rs to keep the core module under 600 lines.

#[cfg(test)]
mod tests {
    use crate::test_utils::{seed_embedding, test_db};
    use std::sync::Arc;

    /// Stress test: insert 1,000 source items in quick succession via
    /// batch_upsert_source_items, then verify all 1,000 are stored and
    /// the total count matches.
    #[test]
    fn stress_bulk_insert_1000_items() {
        let db = test_db();

        let items: Vec<(
            String,
            String,
            Option<String>,
            String,
            String,
            Vec<f32>,
            String,
        )> = (0..1000)
            .map(|i| {
                let source_id = format!("bulk_{}", i);
                let emb = seed_embedding(&format!("stress:{}", i));
                (
                    "stress_test".to_string(),
                    source_id.clone(),
                    Some(format!("https://example.com/{}", i)),
                    format!("Bulk Item {}", i),
                    format!("Content for stress test item number {}", i),
                    emb,
                    "en".to_string(),
                )
            })
            .collect();

        let count = db.batch_upsert_source_items(&items).unwrap();
        assert_eq!(count, 1000, "batch_upsert should process all 1000 items");

        let total = db.total_item_count().unwrap();
        assert_eq!(total, 1000, "DB should contain exactly 1000 items");

        // Verify a few specific items survived the bulk insert
        for check_idx in [0, 499, 999] {
            let exists = db
                .source_item_exists("stress_test", &format!("bulk_{}", check_idx))
                .unwrap();
            assert!(
                exists,
                "Item bulk_{} should exist after bulk insert",
                check_idx
            );
        }

        // Spot-check one item's content is correct
        let item = db
            .get_source_item("stress_test", "bulk_500")
            .unwrap()
            .expect("bulk_500 should exist");
        assert_eq!(item.title, "Bulk Item 500");
        assert_eq!(item.content, "Content for stress test item number 500");
    }

    /// Stress test: spawn multiple threads doing simultaneous reads and
    /// writes on a shared Arc<Database>. Verifies no panics, no deadlocks,
    /// and data integrity under contention.
    #[test]
    fn stress_concurrent_read_write() {
        let db = Arc::new(test_db());
        let num_writers = 4;
        let writes_per_thread = 50;
        let num_readers = 4;

        // Pre-seed some data so readers have something to query
        for i in 0..20 {
            let emb = seed_embedding(&format!("preseed:{}", i));
            db.upsert_source_item(
                "concurrent",
                &format!("preseed_{}", i),
                None,
                &format!("Preseed {}", i),
                "preseed content",
                &emb,
            )
            .unwrap();
        }

        let mut handles = Vec::new();

        // Writer threads
        for t in 0..num_writers {
            let db_clone = Arc::clone(&db);
            handles.push(std::thread::spawn(move || {
                for i in 0..writes_per_thread {
                    let id_str = format!("w{}_{}", t, i);
                    let emb = seed_embedding(&format!("writer:{}:{}", t, i));
                    db_clone
                        .upsert_source_item(
                            "concurrent",
                            &id_str,
                            None,
                            &format!("Thread {} Item {}", t, i),
                            &format!("Content from writer thread {} item {}", t, i),
                            &emb,
                        )
                        .expect("writer upsert should not fail");
                }
            }));
        }

        // Reader threads
        for _ in 0..num_readers {
            let db_clone = Arc::clone(&db);
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    // Read total count — should never error
                    let count = db_clone
                        .total_item_count()
                        .expect("total_item_count should not fail");
                    assert!(
                        count >= 20,
                        "Should always have at least the 20 pre-seeded items"
                    );

                    // Read specific items
                    let _ = db_clone.get_source_items("concurrent", 10);

                    // Check existence
                    let _ = db_clone.source_item_exists("concurrent", "preseed_0");
                }
            }));
        }

        // Wait for all threads to complete (timeout protection via test runner)
        for handle in handles {
            handle.join().expect("thread should not panic");
        }

        // Verify final state: 20 preseeded + (num_writers * writes_per_thread) unique items
        let expected = 20 + (num_writers * writes_per_thread) as i64;
        let actual = db.total_item_count().unwrap();
        assert_eq!(
            actual,
            expected,
            "Final count should be {} (20 preseed + {} writer items), got {}",
            expected,
            num_writers * writes_per_thread,
            actual
        );
    }

    /// Stress test: upsert the same context chunk 100 times with identical
    /// content. Verify only 1 row exists (idempotency via content_hash).
    #[test]
    fn stress_context_upsert_idempotency() {
        let db = test_db();
        let text = "This is a repeated context chunk for idempotency testing.";
        let embedding = seed_embedding("idempotent_context");

        let mut ids = Vec::new();
        for _ in 0..100 {
            let id = db
                .upsert_context("test/idempotent.rs", text, &embedding)
                .unwrap();
            ids.push(id);
        }

        // All upserts should return the same row id
        let first_id = ids[0];
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(
                *id, first_id,
                "Upsert #{} returned id {} but expected {} (first insert)",
                i, id, first_id
            );
        }

        // Only 1 row should exist in context_chunks
        let count = db.context_count().unwrap();
        assert_eq!(
            count, 1,
            "100 upserts of identical content should produce exactly 1 row, got {}",
            count
        );

        // Verify the content is intact
        let contexts = db.get_all_contexts().unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].text, text);
        assert_eq!(contexts[0].source_file, "test/idempotent.rs");
    }

    /// Stress test: store and retrieve a full 384-dimensional embedding
    /// vector. Verify bit-exact roundtrip through the database (f32 -> blob
    /// -> f32 with no precision loss).
    #[test]
    fn stress_large_embedding_roundtrip_384d() {
        let db = test_db();

        // Create a 384-dim embedding with varied values (positive, negative,
        // near-zero, large magnitude) to exercise the full float range.
        let original: Vec<f32> = (0..384)
            .map(|i| {
                let x = i as f32;
                (x * 0.0163 + 0.7).sin() * (1.0 + (x * 0.0071).cos())
            })
            .collect();

        // Store via context upsert
        let id = db
            .upsert_context("embedding_test/large.rs", "384d embedding test", &original)
            .unwrap();
        assert!(id > 0);

        // Retrieve and compare
        let contexts = db.get_all_contexts().unwrap();
        assert_eq!(contexts.len(), 1);
        let recovered = &contexts[0].embedding;

        assert_eq!(
            recovered.len(),
            384,
            "Recovered embedding should be 384-dim, got {}",
            recovered.len()
        );

        // Bit-exact comparison: f32 -> le_bytes -> f32 should be lossless
        for (i, (orig, recv)) in original.iter().zip(recovered.iter()).enumerate() {
            assert_eq!(
                orig.to_bits(),
                recv.to_bits(),
                "Embedding dimension {} differs: original={} recovered={}",
                i,
                orig,
                recv
            );
        }

        // Also test via source_items path for completeness
        let source_emb = seed_embedding("large_emb_source_test");
        assert_eq!(source_emb.len(), 384);

        db.upsert_source_item(
            "embed_test",
            "large_384",
            None,
            "Large Embedding Test",
            "Content",
            &source_emb,
        )
        .unwrap();

        let item = db
            .get_source_item("embed_test", "large_384")
            .unwrap()
            .expect("item should exist");
        assert_eq!(item.embedding.len(), 384);
        for (i, (orig, recv)) in source_emb.iter().zip(item.embedding.iter()).enumerate() {
            assert_eq!(
                orig.to_bits(),
                recv.to_bits(),
                "Source embedding dimension {} differs",
                i
            );
        }
    }

    /// Stress test: insert 100 context items with known embeddings, then
    /// perform KNN search and verify the closest items are returned in the
    /// correct order.
    #[test]
    fn stress_knn_search_accuracy() {
        let db = test_db();

        // Verify vec0 tables exist (sqlite-vec extension was loaded)
        {
            let conn = db.conn.lock();
            let vec_exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='context_vec'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !vec_exists {
                eprintln!("SKIP: stress_knn_search_accuracy — vec0 tables not available (sqlite-vec extension not loaded)");
                return;
            }
        }

        let dim = 384;

        // Create a fixed query vector: unit vector along a specific direction
        let mut query_vec = vec![0.0f32; dim];
        query_vec[0] = 1.0;
        query_vec[1] = 0.5;
        // Normalize
        let norm: f32 = query_vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        for v in &mut query_vec {
            *v /= norm;
        }

        // Insert 100 items with embeddings that have controlled distances from query_vec.
        let mut expected_order: Vec<(i64, f32)> = Vec::new();

        for i in 0..100 {
            let mut emb = query_vec.clone();
            let noise_scale = (i as f32 + 1.0) * 0.05;
            for d in 2..dim {
                emb[d] = noise_scale * ((d as f32 * 0.1 + i as f32 * 0.37).sin());
            }
            emb[0] = query_vec[0] * (1.0 - i as f32 * 0.005);

            // Normalize
            let norm: f32 = emb.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut emb {
                    *v /= norm;
                }
            }

            let id = db
                .upsert_context(
                    &format!("knn_test/item_{}.rs", i),
                    &format!("KNN test item {}", i),
                    &emb,
                )
                .unwrap();

            let dist: f32 = emb
                .iter()
                .zip(query_vec.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                .sqrt();
            expected_order.push((id, dist));
        }

        // Sort by expected distance (ascending)
        expected_order.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Query for top 10 nearest neighbors
        let results = db.find_similar_contexts(&query_vec, 10).unwrap();
        assert_eq!(
            results.len(),
            10,
            "KNN should return exactly 10 results, got {}",
            results.len()
        );

        // Verify results are sorted by ascending distance
        for window in results.windows(2) {
            assert!(
                window[0].distance <= window[1].distance,
                "KNN results should be sorted by distance: {} <= {} violated",
                window[0].distance,
                window[1].distance
            );
        }

        // Verify the top result is indeed the item with smallest expected distance.
        let top_result_id = results[0].context_id;
        let top3_expected_ids: Vec<i64> =
            expected_order.iter().take(3).map(|(id, _)| *id).collect();
        assert!(
            top3_expected_ids.contains(&top_result_id),
            "Top KNN result (id={}) should be among the 3 closest expected items {:?}",
            top_result_id,
            top3_expected_ids
        );

        // The top-10 KNN results should all appear within the top-15 expected items
        let top15_expected_ids: Vec<i64> =
            expected_order.iter().take(15).map(|(id, _)| *id).collect();
        for result in &results {
            assert!(
                top15_expected_ids.contains(&result.context_id),
                "KNN result id={} (dist={:.4}) should be among top-15 expected items",
                result.context_id,
                result.distance
            );
        }

        // Verify the distances are non-negative (L2 distances)
        for result in &results {
            assert!(
                result.distance >= 0.0,
                "KNN distance should be non-negative, got {}",
                result.distance
            );
        }
    }
}
