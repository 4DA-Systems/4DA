// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Centralized runtime path resolution for 4DA.
//!
//! Provides a single source of truth for all filesystem paths, handling
//! both development (CARGO_MANIFEST_DIR) and production (platform-specific) builds.
//! Eliminates scattered `env!("CARGO_MANIFEST_DIR")` calls that break in packaged installs.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::info;

static PATHS: OnceLock<RuntimePaths> = OnceLock::new();

/// All filesystem paths 4DA uses at runtime.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    /// Project root in dev, app data dir in production
    pub data_dir: PathBuf,
    /// Cache directory (safe to delete)
    pub cache_dir: PathBuf,
    /// Bundled read-only resources (translations, docs, models)
    pub resource_dir: PathBuf,
}

#[allow(dead_code)] // Convenience accessors — not all used yet but part of the public API
impl RuntimePaths {
    /// Initialize from the resolved directories. Call once at startup from app_setup.
    ///
    /// In development mode, resolves relative to CARGO_MANIFEST_DIR.
    /// In production, uses platform-specific directories.
    pub fn init() {
        let paths = Self::resolve();
        info!(
            target: "4da::runtime_paths",
            data_dir = %paths.data_dir.display(),
            cache_dir = %paths.cache_dir.display(),
            resource_dir = %paths.resource_dir.display(),
            "Runtime paths initialized"
        );
        let _ = PATHS.set(paths);
    }

    /// Get the initialized paths.
    ///
    /// Auto-initializes on first access if `init()` hasn't been called (e.g. in tests).
    pub fn get() -> &'static RuntimePaths {
        PATHS.get_or_init(Self::resolve)
    }

    /// Resolve paths based on environment.
    fn resolve() -> RuntimePaths {
        // Development: CARGO_MANIFEST_DIR parent is the project root
        let dev_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CARGO_MANIFEST_DIR has no parent")
            .to_path_buf();

        let dev_data = dev_root.join("data");

        // If the dev data dir exists, we're in development mode
        if dev_data.exists() {
            return RuntimePaths {
                data_dir: dev_data,
                cache_dir: dev_root.join("data").join("cache"),
                resource_dir: dev_root,
            };
        }

        // Production: platform-specific directories
        let data_dir = Self::platform_data_dir();
        let cache_dir = Self::platform_cache_dir();

        // Ensure directories exist
        let _ = std::fs::create_dir_all(&data_dir);
        let _ = std::fs::create_dir_all(&cache_dir);

        // Resource dir: in production, bundled resources location varies by platform.
        // - Windows: next to the executable (exe parent)
        // - macOS: AppName.app/Contents/Resources/
        // - Linux: next to the executable or in /usr/share/appname/
        let resource_dir = Self::resolve_resource_dir(&data_dir);

        RuntimePaths {
            data_dir,
            cache_dir,
            resource_dir,
        }
    }

    fn platform_data_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join("com.4da.app").join("data");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                return home
                    .join("Library")
                    .join("Application Support")
                    .join("com.4da.app")
                    .join("data");
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(data_dir) = dirs::data_dir() {
                return data_dir.join("4da").join("data");
            }
        }

        PathBuf::from("data")
    }

    fn platform_cache_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                return PathBuf::from(local).join("com.4da.app").join("cache");
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                return home.join("Library").join("Caches").join("com.4da.app");
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(cache) = dirs::cache_dir() {
                return cache.join("4da");
            }
        }

        PathBuf::from("cache")
    }

    /// Resolve the resource directory based on the current platform.
    ///
    /// On macOS, Tauri bundles resources in `AppName.app/Contents/Resources/`,
    /// not next to the executable. On Windows and Linux, resources are next to
    /// the executable. Falls back to `fallback` if the exe path can't be determined.
    fn resolve_resource_dir(fallback: &Path) -> PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.to_path_buf()));

        if let Some(ref dir) = exe_dir {
            // macOS: check if we're inside an .app bundle
            #[cfg(target_os = "macos")]
            {
                // In a macOS .app bundle, the exe is at:
                //   AppName.app/Contents/MacOS/appname
                // Resources are at:
                //   AppName.app/Contents/Resources/
                if let Some(macos_dir) = dir.parent() {
                    let resources = macos_dir.join("Resources");
                    if resources.exists() {
                        return resources;
                    }
                }
            }

            // Windows and Linux: resources are next to the executable
            return dir.clone();
        }

        // Fallback to data_dir if exe path can't be determined
        fallback.to_path_buf()
    }

    // ── Convenience accessors ──────────────────────────────────────

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("4da.db")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn exports_dir(&self) -> PathBuf {
        let dir = self.data_dir.join("exports");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    pub fn model_cache_dir(&self) -> PathBuf {
        let dir = self.cache_dir.join("models");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Localization files — bundled with the app
    pub fn locales_dir(&self) -> PathBuf {
        // Dev: project_root/locales, Prod: resource_dir/locales
        self.resource_dir.join("locales")
    }

    /// STREETS playbook docs — bundled with the app
    pub fn streets_docs_dir(&self) -> PathBuf {
        self.resource_dir.join("docs").join("streets")
    }

    /// STREETS regions for price tracker — bundled with the app
    pub fn streets_regions_dir(&self) -> PathBuf {
        self.resource_dir
            .join("docs")
            .join("streets")
            .join("regions")
    }

    /// OCR models directory — bundled with the app
    pub fn ocr_models_dir(&self) -> PathBuf {
        // Dev: src-tauri/models, Prod: resource_dir/models
        if cfg!(debug_assertions) {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")
        } else {
            self.resource_dir.join("models")
        }
    }
}
