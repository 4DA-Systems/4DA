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
//! The app (`src-tauri`) keeps the DB and network halves and re-exports these
//! under their old `crate::osv::...` paths.

pub mod api;
pub mod ecosystem;
pub mod fix_target;
pub mod range;
pub mod version_order;

#[cfg(target_arch = "wasm32")]
mod wasm;
