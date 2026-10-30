// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! One strict-mode OSV item per advisory.
//!
//! Both strict lanes (live per-version query and the mirror matcher) emit one item per
//! (advisory, matched package). An advisory can name several packages a stack has: Go
//! records name the toolchain (`stdlib`, matched by the go.mod `go` directive) and a module
//! such as golang.org/x/net. The store keys items by `("osv", advisory_id)`, so the copies
//! overwrote each other and the title named whichever came last; the fetcher then reuses the
//! stored title on every later run. GO-2026-6603 and four siblings were stored as
//! "stdlib: ... in net/http" and the pinned x/net copy was never named.

use std::collections::HashMap;

use super::SourceItem;

/// Collapse items sharing an advisory id into one, in first-seen order. The kept copy names
/// a package the stack pins before the Go toolchain pseudo-packages (`stdlib`, `toolchain`),
/// then a version-confirmed copy before a conservative one, then the first seen. The other
/// matched packages are listed in its content and in `metadata.packages`.
pub(super) fn one_item_per_advisory(items: Vec<SourceItem>) -> Vec<SourceItem> {
    let mut groups: Vec<Vec<SourceItem>> = Vec::new();
    let mut index: HashMap<(String, String), usize> = HashMap::new();
    for item in items {
        let key = (item.source_type.clone(), item.source_id.clone());
        match index.get(&key).and_then(|&at| groups.get_mut(at)) {
            Some(copies) => copies.push(item),
            None => {
                index.insert(key, groups.len());
                groups.push(vec![item]);
            }
        }
    }
    groups.into_iter().filter_map(merge_copies).collect()
}

fn merge_copies(mut copies: Vec<SourceItem>) -> Option<SourceItem> {
    if copies.len() < 2 {
        return copies.pop();
    }
    let lead_at = copies
        .iter()
        .enumerate()
        .min_by_key(|(at, copy)| (is_go_toolchain(copy), !is_confirmed(copy), *at))
        .map(|(at, _)| at)?;
    let mut lead = copies.remove(lead_at);
    let mut packages: Vec<String> = meta_str(&lead, "package").into_iter().collect();
    let mut also = Vec::new();
    for copy in &copies {
        let Some(package) = meta_str(copy, "package") else {
            continue;
        };
        if packages.iter().any(|p| p.eq_ignore_ascii_case(&package)) {
            continue;
        }
        also.push(match meta_str(copy, "installed_version") {
            Some(version) => format!("{package} {version}"),
            None => package.clone(),
        });
        packages.push(package);
    }
    if !also.is_empty() {
        lead.content
            .push_str(&format!("\nAlso affects: {}", also.join(", ")));
    }
    if let Some(serde_json::Value::Object(meta)) = lead.metadata.as_mut() {
        meta.insert("packages".to_string(), serde_json::json!(packages));
    }
    Some(lead)
}

/// Heal an OSV item stored before [`one_item_per_advisory`]: the fetcher keeps a stored
/// item's title on every re-see, so a title written as `[id] stdlib: ...` would stand
/// forever even though the collapsed item now names the pinned module. When the stored
/// title leads with a Go toolchain pseudo-package and the fresh one for the same id leads
/// with something else, write the fresh title and return it. One-shot and one-way: once
/// rewritten the stored title no longer leads with the toolchain, and a module title is
/// never turned back into a toolchain one.
pub(crate) fn heal_toolchain_title(
    db: &crate::db::Database,
    source_type: &str,
    source_id: &str,
    stored_title: &str,
    fresh_title: &str,
) -> Option<String> {
    if source_type != "osv"
        || !leads_with_go_toolchain(stored_title)
        || leads_with_go_toolchain(fresh_title)
        || !fresh_title.starts_with(&format!("[{source_id}] "))
    {
        return None;
    }
    let conn = db.conn.lock();
    match conn.execute(
        "UPDATE source_items SET title = ?3 WHERE source_type = ?1 AND source_id = ?2",
        rusqlite::params![source_type, source_id, fresh_title],
    ) {
        Ok(1) => Some(fresh_title.to_string()),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(target: "4da::sources", source_id, error = %e, "OSV title heal failed");
            None
        }
    }
}

/// `[id] stdlib: ...` / `[id] toolchain: ...`: the title names the Go toolchain.
fn leads_with_go_toolchain(title: &str) -> bool {
    let body = title
        .split_once("] ")
        .map_or(title, |(_, rest)| rest)
        .trim_start();
    body.starts_with("stdlib:") || body.starts_with("toolchain:")
}

/// OSV's Go ecosystem names the standard library `stdlib` and the go command `toolchain`.
fn is_go_toolchain(item: &SourceItem) -> bool {
    let go = meta_str(item, "ecosystem").is_some_and(|e| e.eq_ignore_ascii_case("go"));
    go && meta_str(item, "package").is_some_and(|p| p == "stdlib" || p == "toolchain")
}

fn is_confirmed(item: &SourceItem) -> bool {
    item.metadata
        .as_ref()
        .and_then(|m| m.get("is_version_confirmed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn meta_str(item: &SourceItem, key: &str) -> Option<String> {
    item.metadata
        .as_ref()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::osv_live::live_vuln_to_grounded_item;
    use crate::sources::osv_types::OsvVulnerability;

    /// The conformance corpus's pinned copy, byte-for-byte as served by OSV.
    fn go_2026_6603() -> OsvVulnerability {
        serde_json::from_str(include_str!(
            "../../tests/conformance/osv/GO-2026-6603.json"
        ))
        .expect("pinned OSV record parses")
    }

    #[test]
    fn go_advisory_is_one_item_led_by_the_pinned_module_in_either_order() {
        let vuln = go_2026_6603();
        let xnet = || live_vuln_to_grounded_item(&vuln, "golang.org/x/net", "Go", "0.21.0");
        let stdlib = || live_vuln_to_grounded_item(&vuln, "stdlib", "Go", "1.22.0");
        for items in [vec![xnet(), stdlib()], vec![stdlib(), xnet()]] {
            let out = one_item_per_advisory(items);
            let titles: Vec<&str> = out.iter().map(|i| i.title.as_str()).collect();
            assert_eq!(out.len(), 1, "one item per advisory id, got {titles:?}");
            let item = &out[0];
            assert_eq!(item.source_id, "GO-2026-6603");
            assert!(
                item.title.starts_with("[GO-2026-6603] golang.org/x/net: "),
                "the title leads with the pinned module, got {:?}",
                item.title
            );
            assert!(
                item.content.contains("Installed: 0.21.0"),
                "{}",
                item.content
            );
            assert!(
                item.content.contains("Fixed in: 0.60.0"),
                "{}",
                item.content
            );
            assert!(
                item.content.contains("Also affects: stdlib 1.22.0"),
                "{}",
                item.content
            );
            let meta = item.metadata.as_ref().expect("metadata");
            assert_eq!(meta["package"], "golang.org/x/net");
            assert_eq!(
                meta["packages"],
                serde_json::json!(["golang.org/x/net", "stdlib"])
            );
        }
    }

    /// The five go-service rows were stored on 2026-10-08 as `stdlib: ...`. After the collapse
    /// the fresh item names x/net; the stored title must follow, once, and only that way.
    #[test]
    fn stored_toolchain_title_heals_to_the_collapsed_module_title_once() {
        let db = crate::test_utils::test_db();
        let vuln = go_2026_6603();
        let stored = live_vuln_to_grounded_item(&vuln, "stdlib", "Go", "1.22.0").title;
        let fresh = one_item_per_advisory(vec![
            live_vuln_to_grounded_item(&vuln, "golang.org/x/net", "Go", "0.21.0"),
            live_vuln_to_grounded_item(&vuln, "stdlib", "Go", "1.22.0"),
        ])
        .remove(0)
        .title;
        db.conn
            .lock()
            .execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding)
                 VALUES ('osv', 'GO-2026-6603', ?1, '', 'h', X'')",
                [&stored],
            )
            .unwrap();
        let read = |db: &crate::db::Database| -> String {
            db.conn
                .lock()
                .query_row(
                    "SELECT title FROM source_items WHERE source_id = 'GO-2026-6603'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };

        let healed = heal_toolchain_title(&db, "osv", "GO-2026-6603", &stored, &fresh);
        assert_eq!(healed.as_deref(), Some(fresh.as_str()));
        assert!(read(&db).starts_with("[GO-2026-6603] golang.org/x/net: "));
        // Idempotent: the stored title no longer leads with the toolchain.
        assert_eq!(
            heal_toolchain_title(&db, "osv", "GO-2026-6603", &fresh, &fresh),
            None
        );
        // One-way: a module title is never turned back into a toolchain one.
        assert_eq!(
            heal_toolchain_title(&db, "osv", "GO-2026-6603", &fresh, &stored),
            None
        );
        assert_eq!(read(&db), fresh);
        // Other sources, and a fresh title for a different id, are left alone.
        assert_eq!(
            heal_toolchain_title(&db, "cve", "GO-2026-6603", &stored, &fresh),
            None
        );
        assert_eq!(
            heal_toolchain_title(&db, "osv", "GO-2026-6610", &stored, &fresh),
            None
        );
    }

    fn item(id: &str, package: &str, confirmed: bool) -> SourceItem {
        SourceItem::new("osv", id, &format!("[{id}] {package}: s")).with_metadata(
            serde_json::json!({
                "ecosystem": "Go",
                "package": package,
                "installed_version": "1.0.0",
                "is_version_confirmed": confirmed,
            }),
        )
    }

    #[test]
    fn toolchain_only_and_distinct_advisories_keep_their_items() {
        let out = one_item_per_advisory(vec![
            item("GO-A", "stdlib", true),
            item("GO-B", "example.com/a", false),
            item("GO-C", "example.com/c", true),
            item("GO-B", "example.com/b", true),
        ]);
        let titles: Vec<&str> = out.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "[GO-A] stdlib: s",
                "[GO-B] example.com/b: s",
                "[GO-C] example.com/c: s"
            ],
            "toolchain-only keeps its title; a confirmed copy leads; first-seen order"
        );
    }
}
