// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Typed wrappers for external binaries and HTTP APIs.
//!
//! This module is **Layer 1** of the Silent-Failure Defense Architecture
//! (see `docs/strategy/SILENT-FAILURE-DEFENSE.md`). It enforces contract
//! verification at every boundary 4DA crosses, by making every call go
//! through a typed wrapper that returns `Result<TypedOutput, TypedError>`
//! and cannot yield a success value without passing all contract checks.
//!
//! ## What "silent failure" means here
//!
//! A silent failure is a bug where side A of a boundary completes
//! "successfully" (exit code 0, HTTP 200, function returned `Ok`) but
//! side B's intended outcome is not implied by that local success.
//!
//! ## How this module prevents the class
//!
//! 1. **Raw `Command::new` for known external binaries should live ONLY
//!    inside this module.** A future lint / validator
//!    (`scripts/validate-boundary-calls.cjs`) enforces this at commit time.
//!
//! 2. **Every wrapper method performs mandatory contract verification**
//!    before returning a typed success value.
//!
//! 3. **Typed errors classify failure modes.** Rather than returning
//!    `Result<_, String>`, each wrapper defines a typed error enum
//!    that forces the caller to pattern-match on failure categories.
//!    This surfaces failure modes at compile time.
//!
//! 4. **Integration tests run the real binary** (see
//!    `src-tauri/tests/integration/` when wired up). Mocks are forbidden
//!    as the only test for a wrapper — they hide contract drift.
//!
//! ## Not in scope
//!
//! - Rust <-> SQLite (different boundary class — defense is schema drift
//!   detection + `PRAGMA integrity_check`)
//! - Rust <-> Filesystem (handled by `RuntimePaths` + watchdog markers)
//! - Rust <-> Tauri IPC (handled by `validate-commands.cjs`)

// Future modules:
// pub mod ollama;  // HTTP client for Ollama /api/* endpoints
// pub mod git;     // Git operations (currently inline in ace/git.rs)
