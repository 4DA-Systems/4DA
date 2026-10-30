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
