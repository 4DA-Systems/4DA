// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `bun.lock` (Bun >= 1.2 text lockfile).
//!
//! JSONC with trailing commas. `workspaces` lists each workspace's declared
//! dependencies; `packages` maps a resolution key to
//! `["name@version", registry, { dependencies, optionalDependencies, … }, integrity]`.
//! The key is the package name for the hoisted copy and a path for a nested
//! one (`next/postcss` is the postcss `next` resolves to; scoped names keep
//! their slash: `@tanstack/router-plugin/@babel/core`). Workspace, link,
//! file and git entries name no registry version. There was no reader at
//! all before 2026-10-10: hono's 862-package bun.lock contributed nothing.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::Value;

use super::npm::{is_registry_version, split_name_at_version};
use super::{DepScope, LockFormat, LockedPackage, LockfileRead};

/// JSONC -> JSON: drop trailing commas outside strings (bun.lock has no comments).
pub(super) fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.get(i + 1) {
                    out.push(*next);
                    i += 1;
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == ',' && next_significant(&chars, i + 1).is_some_and(|n| n == '}' || n == ']')
        {
            // trailing comma: dropped
        } else {
            in_string = c == '"';
            out.push(c);
        }
        i += 1;
    }
    out
}

fn next_significant(chars: &[char], from: usize) -> Option<char> {
    chars[from.min(chars.len())..]
        .iter()
        .copied()
        .find(|c| !c.is_whitespace())
}

/// `next/@babel/core` -> `["next", "@babel/core"]`: a scope and its name are one segment.
fn key_segments(key: &str) -> Vec<String> {
    let parts: Vec<&str> = key.split('/').collect();
    let mut segments = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        if parts[i].starts_with('@') && i + 1 < parts.len() {
            segments.push(format!("{}/{}", parts[i], parts[i + 1]));
            i += 2;
        } else {
            segments.push(parts[i].to_string());
            i += 1;
        }
    }
    segments
}

struct Entry {
    name: String,
    version: String,
    deps: Vec<String>,
}

fn entry(value: &Value) -> Option<Entry> {
    let spec = value.as_array()?.first()?.as_str()?;
    let (name, version) = split_name_at_version(spec)?;
    if !is_registry_version(version) {
        return None;
    }
    let meta = value.as_array()?.get(2);
    let mut deps = Vec::new();
    for field in ["dependencies", "optionalDependencies"] {
        if let Some(map) = meta.and_then(|m| m.get(field)).and_then(Value::as_object) {
            deps.extend(map.keys().cloned());
        }
    }
    Some(Entry {
        name: name.to_string(),
        version: version.to_string(),
        deps,
    })
}

/// Node-style resolution: the nearest nested copy walking up the key, else the hoisted one.
fn resolve<'a>(entries: &'a HashMap<String, Entry>, from: &str, dep: &str) -> Option<&'a str> {
    let segments = key_segments(from);
    for n in (1..=segments.len()).rev() {
        let candidate = format!("{}/{}", segments[..n].join("/"), dep);
        if let Some((key, _)) = entries.get_key_value(&candidate) {
            return Some(key.as_str());
        }
    }
    entries.get_key_value(dep).map(|(k, _)| k.as_str())
}

fn reach(entries: &HashMap<String, Entry>, roots: &HashSet<String>) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack: Vec<String> = roots
        .iter()
        .filter(|r| entries.contains_key(*r))
        .cloned()
        .collect();
    while let Some(key) = stack.pop() {
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(e) = entries.get(&key) {
            for dep in &e.deps {
                if let Some(next) = resolve(entries, &key, dep) {
                    if !seen.contains(next) {
                        stack.push(next.to_string());
                    }
                }
            }
        }
    }
    seen
}

/// The runtime and dev roots every workspace declares.
fn workspace_roots(lock: &Value) -> (HashSet<String>, HashSet<String>) {
    let mut runtime = HashSet::new();
    let mut dev = HashSet::new();
    let workspaces = lock.get("workspaces").and_then(Value::as_object);
    for ws in workspaces.into_iter().flat_map(|w| w.values()) {
        let names = |field: &str| {
            ws.get(field)
                .and_then(Value::as_object)
                .map(|map| map.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        runtime.extend(names("dependencies"));
        runtime.extend(names("optionalDependencies"));
        dev.extend(names("devDependencies"));
    }
    (runtime, dev)
}

pub(super) fn read_bun_lock(content: &str) -> Result<LockfileRead, String> {
    let lock: Value = serde_json::from_str(&strip_trailing_commas(content))
        .map_err(|e| format!("invalid bun.lock JSONC: {e}"))?;
    let packages = lock
        .get("packages")
        .and_then(Value::as_object)
        .ok_or_else(|| "no `packages` section".to_string())?;
    let mut non_registry = 0usize;
    let mut entries: HashMap<String, Entry> = HashMap::new();
    for (key, value) in packages {
        match entry(value) {
            Some(e) => {
                entries.insert(key.clone(), e);
            }
            None => non_registry += 1,
        }
    }
    let (runtime_roots, dev_roots) = workspace_roots(&lock);
    let runtime = reach(&entries, &runtime_roots);
    let dev = reach(&entries, &dev_roots);
    let mut keys: Vec<&String> = entries.keys().collect();
    keys.sort();
    let packages = keys
        .into_iter()
        .filter_map(|key| {
            let e = entries.get(key)?;
            let scope = if runtime.contains(key) {
                DepScope::Runtime
            } else if dev.contains(key) {
                DepScope::Dev
            } else {
                DepScope::Unknown
            };
            Some(
                LockedPackage::new(e.name.clone(), e.version.clone())
                    .scoped(scope)
                    .primary(*key == e.name),
            )
        })
        .collect();
    Ok(LockfileRead {
        path: PathBuf::new(),
        format: LockFormat::Bun,
        packages,
        edges: Vec::new(),
        non_registry_entries: non_registry,
    })
}
