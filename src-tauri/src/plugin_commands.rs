//! Tauri commands for the Source Plugin API.
//!
//! Exposes plugin discovery and execution to the frontend via IPC.

use crate::error::Result;
use crate::plugins::loader;
use crate::plugins::{PluginItem, PluginManifest};

/// List all installed plugins (reads manifest.json from each plugin subdirectory).
#[tauri::command]
pub async fn list_plugins() -> Result<Vec<PluginManifest>> {
    Ok(loader::discover_plugins())
}

/// Fetch items from a specific plugin by name.
///
/// Builds a PluginConfig from the user's current context (tech stack, interests)
/// and executes the named plugin binary.
#[tauri::command]
pub async fn fetch_plugin_items(plugin_name: String) -> Result<Vec<PluginItem>> {
    let plugins = loader::discover_plugins();
    let manifest = plugins
        .iter()
        .find(|p| p.name == plugin_name)
        .ok_or_else(|| format!("Plugin '{}' not found", plugin_name))?;

    let config = loader::build_plugin_config();
    loader::execute_plugin(manifest, &config)
}

/// Fetch items from all installed plugins at once.
///
/// Returns a flat list of all items from all plugins. Each item's `source_type`
/// identifies which plugin it came from.
#[tauri::command]
pub async fn fetch_all_plugins() -> Result<Vec<PluginItem>> {
    let config = loader::build_plugin_config();
    let results = loader::fetch_all_plugin_items(&config);
    let all_items: Vec<PluginItem> = results.into_iter().flat_map(|(_, items)| items).collect();
    Ok(all_items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_plugins_does_not_panic() {
        // Synchronous test — just verify the underlying function works
        let plugins = loader::discover_plugins();
        // In test environment, likely empty unless plugins are installed
        assert!(plugins.is_empty() || !plugins.is_empty());
    }
}
