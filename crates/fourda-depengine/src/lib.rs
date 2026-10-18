// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `fourda-depengine`: the pure core of 4DA's dependency engine (single-engine
//! plan, Stage C0).
//!
//! Strings and structs in, structs out. Nothing in this crate reads a file,
//! opens a socket, touches a database or reads the clock, so the same code
//! runs inside the desktop app and as a `wasm32-unknown-unknown` module with
//! zero imports.
//!
//! C0.1 holds the rules every finding and every fix depends on:
//! - [`version_order`]: one ordering primitive (semver, PEP 440, OSV's
//!   `introduced: "0"`);
//! - [`range`]: whether a version sits in an advisory's affected windows;
//! - [`fix_target`]: the fix for a version's release line, and the walk to the
//!   lowest version that leaves every window with a way out.
//!
//! C0.2 adds inventory matching, split from the app's database:
//! - [`osv`]: a verbatim OSV record as the mirror stores it (one row per
//!   package, `affected` entries merged);
//! - [`matching`]: `match_inventory(&[Package], &[AdvisoryRecord])`, the
//!   findings per installed copy with each copy's fix targets;
//! - [`exposure`]: the same check for one advisory (the scorer's verdict);
//! - [`lines`]: the per-project, per-release-line upgrade targets;
//! - [`malware`]: whether an advisory is a malicious-package record (`MAL-`
//!   id or alias, CWE-506, or GitHub's anchored malware title template);
//! - [`registry`]: the registry's releases and the ones it withdrew (yanked
//!   or deprecated), which no target may name.
//!
//! The app (`src-tauri`) keeps the DB and network halves and re-exports these
//! under their old `crate::osv::...` paths.

pub mod api;
pub mod ecosystem;
pub mod exposure;
pub mod fix_target;
pub mod lines;
pub mod malware;
pub mod matching;
pub mod osv;
pub mod range;
pub mod registry;
pub mod version_order;

pub use malware::is_malware;

#[cfg(target_arch = "wasm32")]
mod wasm;
