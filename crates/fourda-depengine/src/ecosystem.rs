// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The subset of the app's ecosystem normalisation (`src-tauri/src/ecosystem.rs`)
//! the pure rules need. A parity test in the app
//! (`osv::depengine_parity_tests`) holds the two alias sets together.

/// Whether `ecosystem` names PyPI, under any alias the app uses
/// (`Ecosystem::parse(..).osv_name() == "PyPI"` in the app).
pub fn is_pypi(ecosystem: &str) -> bool {
    matches!(
        ecosystem.trim().to_lowercase().as_str(),
        "python" | "pypi" | "pip" | "py"
    )
}
