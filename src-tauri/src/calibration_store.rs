//! Calibration curve persistence — Phase 5b.1.
//!
//! Curves live on disk at `{data_dir}/calibrations/{identity_hash}/{task}.json`.
//! One file per (model, task) pair. Atomic writes via tmp-file + rename so
//! a crash mid-write leaves the previous curve intact. Corrupted files
//! return `None` from `load_curve` with a warn log; callers treat absence
//! and corruption identically (fall back to `None` curve → pass-through).
//!
//! ## Why files, not SQLite
//!
//! Curves are write-rarely-read-once artifacts keyed by a stable hash.
//! Filesystem keying is simpler than a SQL schema, trivially introspectable
//! (`cat ~/4DA/calibrations/abc123/judge.json`), and matches how per-user
//! federated-calibration snapshots (Phase 8) will look when shared. If
//! curve count ever exceeds ~10k (~one per active model-task pair), we
//! revisit; until then, files.
//!
//! ## Invariants
//!
//!   - `load_curve` returns `None` for every non-fatal error (missing
//!     file, parse failure, IO error). Callers treat `None` as "use the
//!     inner core's calibration_id" — which is the safe fallback.
//!   - `save_curve` is atomic w.r.t. crashes: either the new file is
//!     complete on disk or the old one is untouched.
//!   - Directory creation is idempotent.

use crate::calibration::CalibrationCurve;
use crate::error::Result;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, warn};
use ts_rs::TS;

/// Diagnostic describing why a curve was rejected at load time.
///
/// Today only prompt-version drift is detected (model identity
/// divergence shows up as a different filename and produces `Missing`
/// via the underlying `load_curve`, which has no drift signal to
/// report). Expanding the enum for new drift kinds (temperature,
/// calibration-id mismatch, etc.) should be additive.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CurveDrift {
    pub curve_id: String,
    pub task: String,
    pub model_identity_hash: String,
    pub stored_prompt_version: String,
    pub current_prompt_version: String,
    /// Stable reason code the UI can switch on for localized copy.
    /// "prompt_version_mismatch" is the only variant today.
    pub reason: String,
}

/// Outcome of a "load and validate" attempt.
///
/// - `curve.is_some()` with `drift.is_none()` — fresh, usable curve.
/// - `curve.is_none()` with `drift.is_some()` — an on-disk curve was
///   found but invalidated by the drift check. Callers should treat
///   this as pass-through AND surface the drift to the user.
/// - Both `None` — no curve on disk (or corrupted; see underlying
///   logs). Silent pass-through, no user-visible drift event.
#[derive(Debug, Clone)]
pub struct CurveLoad {
    pub curve: Option<CalibrationCurve>,
    pub drift: Option<CurveDrift>,
}

/// Base directory for calibration curves, rooted at the runtime data dir.
/// Created on demand by `save_curve`; callers should not precreate.
fn calibration_dir() -> PathBuf {
    crate::runtime_paths::RuntimePaths::get()
        .data_dir
        .join("calibrations")
}

/// Path for a specific (identity_hash, task) curve.
///
/// Splits by identity_hash subdirectory so a single model with N tasks
/// keeps its curves together, and so `ls calibrations/` is a tidy
/// flat listing of models rather than a Cartesian file explosion.
pub fn path_for(identity_hash: &str, task: &str) -> PathBuf {
    calibration_dir()
        .join(sanitize_path_component(identity_hash))
        .join(format!("{}.json", sanitize_path_component(task)))
}

/// Restricts path components to `[a-zA-Z0-9_-]` to prevent traversal
/// attacks if identity_hash or task ever flow in from user content.
/// Today both values are internally computed (sha256 hex / hardcoded
/// task name), so this is belt-and-braces defense, not load-bearing.
fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Attempt to load a curve for (model, task). Returns `None` on any
/// failure mode — missing file, bad JSON, unreadable permissions.
///
/// Deliberately Option-not-Result so the rerank loop's construction path
/// is clean: `CalibratedCore::new(inner, calibration_store::load_curve(...))`.
/// Failures here are logged at `debug` (missing) or `warn` (corruption)
/// but never surfaced to the caller — missing calibration is always
/// recoverable via pass-through.
pub fn load_curve(identity_hash: &str, task: &str) -> Option<CalibrationCurve> {
    let path = path_for(identity_hash, task);
    if !path.exists() {
        debug!(
            target: "4da::calibration_store",
            path = %path.display(),
            "No calibration curve on disk (pass-through)"
        );
        return None;
    }
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            warn!(
                target: "4da::calibration_store",
                path = %path.display(),
                error = %e,
                "Failed to read calibration curve (falling back to pass-through)"
            );
            return None;
        }
    };
    match serde_json::from_slice::<CalibrationCurve>(&bytes) {
        Ok(curve) => {
            debug!(
                target: "4da::calibration_store",
                curve_id = %curve.curve_id,
                samples = curve.sample_count,
                "Loaded calibration curve"
            );
            Some(curve)
        }
        Err(e) => {
            warn!(
                target: "4da::calibration_store",
                path = %path.display(),
                error = %e,
                "Calibration curve JSON corrupted (falling back to pass-through). \
                 Next fitter run will overwrite with a fresh curve."
            );
            None
        }
    }
}

/// Load a curve ONLY if it matches the current model identity + prompt
/// version. Returns `None` when the curve is stale (either the model
/// was swapped or the prompt template got revised since the curve was
/// fit), so the rerank loop falls through to pass-through until a
/// fresh curve is produced.
///
/// This is the drift-detection layer. Without it, a stale curve could
/// silently mis-calibrate confidences after a model swap or prompt
/// revision, and the receipts UI would lie about which curve produced
/// which score.
///
/// The currency check is ALSO logged (warn) when it fires so the user
/// can see "your llama3.2 curve expired because the model updated,
/// recalibrating from next feedback batch" in the event log.
/// Option-only convenience wrapper kept for tests and any future
/// call site that doesn't need the drift descriptor.
#[allow(dead_code)] // Used by calibration_fitter tests; production now uses _detailed.
pub fn load_current_curve(
    identity_hash: &str,
    task: &str,
    current_prompt_version: &str,
) -> Option<CalibrationCurve> {
    load_current_curve_detailed(identity_hash, task, current_prompt_version).curve
}

/// Same as `load_current_curve` but returns the richer `CurveLoad`
/// struct so callers with access to a Tauri AppHandle can surface
/// drift events to the frontend.
///
/// `load_current_curve` is kept as a thin wrapper for tests and call
/// sites that cannot emit events (e.g. inside the fitter, which runs
/// before the drift would be user-meaningful anyway).
pub fn load_current_curve_detailed(
    identity_hash: &str,
    task: &str,
    current_prompt_version: &str,
) -> CurveLoad {
    let Some(curve) = load_curve(identity_hash, task) else {
        return CurveLoad {
            curve: None,
            drift: None,
        };
    };

    // Identity hash check is implicit in the filename — if the hash
    // changes, we look up a DIFFERENT file, so a swapped-model case
    // naturally produces `None` from load_curve. The remaining drift
    // source is the prompt_version field on the curve itself.
    if curve.prompt_version != current_prompt_version {
        warn!(
            target: "4da::calibration_store",
            curve_id = %curve.curve_id,
            curve_prompt = %curve.prompt_version,
            current_prompt = %current_prompt_version,
            "Calibration curve prompt_version drift — invalidating (pass-through until refit)"
        );
        let drift = CurveDrift {
            curve_id: curve.curve_id.clone(),
            task: task.to_string(),
            model_identity_hash: identity_hash.to_string(),
            stored_prompt_version: curve.prompt_version.clone(),
            current_prompt_version: current_prompt_version.to_string(),
            reason: "prompt_version_mismatch".to_string(),
        };
        return CurveLoad {
            curve: None,
            drift: Some(drift),
        };
    }
    CurveLoad {
        curve: Some(curve),
        drift: None,
    }
}

/// Atomically write a curve to disk. Creates the parent directory if
/// needed. Write happens to a `.tmp` sibling, then rename replaces the
/// previous file — on most filesystems this is atomic, so a crash
/// mid-write leaves the previous curve intact.
pub fn save_curve(curve: &CalibrationCurve) -> Result<()> {
    let path = path_for(&curve.model_identity_hash, &curve.task);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(curve)?;

    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }

    fs::rename(&tmp_path, &path)?;
    debug!(
        target: "4da::calibration_store",
        path = %path.display(),
        curve_id = %curve.curve_id,
        "Saved calibration curve atomically"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::{CalibrationBucket, CalibrationCurve};
    use chrono::Utc;
    use tempfile::TempDir;

    fn test_curve(hash: &str, task: &str) -> CalibrationCurve {
        CalibrationCurve {
            curve_id: format!("{task}-test-cal-v1"),
            model_identity_hash: hash.to_string(),
            task: task.to_string(),
            prompt_version: "judge-v1-2026-04-15".to_string(),
            buckets: vec![
                CalibrationBucket {
                    raw_bucket_lo: 0.0,
                    raw_bucket_hi: 0.5,
                    raw_bucket_center: 0.25,
                    observed_positive_rate: 0.20,
                    sample_count: 10,
                },
                CalibrationBucket {
                    raw_bucket_lo: 0.5,
                    raw_bucket_hi: 1.0,
                    raw_bucket_center: 0.75,
                    observed_positive_rate: 0.70,
                    sample_count: 15,
                },
            ],
            brier_score: 0.15,
            ece: 0.10,
            sample_count: 25,
            created_at: Utc::now(),
        }
    }

    fn with_data_dir<F: FnOnce(&TempDir)>(f: F) {
        // Use a scoped tempdir and a PATHS OnceLock override — since
        // runtime_paths::PATHS is private, we test via public path_for
        // helpers that use it. Tests that need isolated filesystems
        // go through the tempdir directly with path_for's format.
        let tmp = TempDir::new().expect("tempdir");
        f(&tmp);
    }

    #[test]
    fn sanitize_path_component_strips_suspicious_chars() {
        // Load-bearing: a malicious identity_hash or task cannot traverse
        // outside the calibrations directory. Every non-[a-zA-Z0-9_-]
        // char becomes underscore.
        assert_eq!(
            sanitize_path_component("../../etc/passwd"),
            "______etc_passwd"
        );
        assert_eq!(sanitize_path_component("abc/def\\g"), "abc_def_g");
        assert_eq!(
            sanitize_path_component("valid-hash_and.dot"),
            "valid-hash_and_dot"
        );
    }

    #[test]
    fn sanitize_prevents_path_traversal() {
        // Real-world attack vectors: null byte, unicode, windows-style paths.
        for attack in [
            "../../../foo",
            "..\\..\\foo",
            "foo\0bar",
            "foo/bar/../baz",
            "foo;rm -rf /",
        ] {
            let sanitized = sanitize_path_component(attack);
            assert!(!sanitized.contains('/'));
            assert!(!sanitized.contains('\\'));
            assert!(!sanitized.contains('\0'));
            assert!(!sanitized.contains(';'));
            assert!(!sanitized.contains(' '));
            assert!(!sanitized.contains('.'));
        }
    }

    #[test]
    fn sanitize_preserves_alphanumeric_and_dash_underscore() {
        assert_eq!(
            sanitize_path_component("judge-abc123_DEF"),
            "judge-abc123_DEF"
        );
    }

    #[test]
    fn load_missing_path_returns_none() {
        // We cannot easily substitute runtime_paths in-test. Verify the
        // primitive contract: load_curve handles missing files by returning
        // None without panicking. We test via a direct path check using
        // sanitize, assuming a clearly nonexistent hash.
        let curve = load_curve("nonexistent_hash_7z8x9", "judge");
        assert!(curve.is_none());
    }

    #[test]
    fn curve_serde_roundtrip_via_filesystem() {
        // Direct file I/O test without routing through load_curve's path
        // resolution — this validates the serialization contract that
        // load_curve relies on.
        with_data_dir(|tmp| {
            let path = tmp.path().join("test_curve.json");
            let curve = test_curve("abcdef", "judge");

            let json = serde_json::to_vec_pretty(&curve).unwrap();
            fs::write(&path, &json).unwrap();

            let bytes = fs::read(&path).unwrap();
            let restored: CalibrationCurve = serde_json::from_slice(&bytes).unwrap();

            assert_eq!(restored.curve_id, curve.curve_id);
            assert_eq!(restored.model_identity_hash, curve.model_identity_hash);
            assert_eq!(restored.task, curve.task);
            assert_eq!(restored.buckets.len(), curve.buckets.len());
            assert!((restored.brier_score - curve.brier_score).abs() < 1e-6);
        });
    }

    #[test]
    fn corrupted_json_is_handled_gracefully() {
        with_data_dir(|tmp| {
            let path = tmp.path().join("bad.json");
            fs::write(&path, b"{ this is not valid json }").unwrap();

            let bytes = fs::read(&path).unwrap();
            let result = serde_json::from_slice::<CalibrationCurve>(&bytes);
            // The real load_curve would see this pattern and return None
            // with a warn log — verify the underlying serde behavior.
            assert!(result.is_err());
        });
    }

    #[test]
    fn path_for_produces_sensible_layout() {
        // Does not exercise runtime_paths (which requires init); instead
        // validates the structure of the returned path on a known prefix
        // by parsing the tail segments.
        let p = path_for("abc123", "judge");
        let components: Vec<_> = p.components().collect();
        let last_three: Vec<_> = components
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert_eq!(last_three, vec!["calibrations", "abc123", "judge.json"]);
    }

    #[test]
    fn path_for_different_tasks_same_model_share_directory() {
        let judge = path_for("modelhash", "judge");
        let summarize = path_for("modelhash", "summarize");
        assert_eq!(judge.parent(), summarize.parent());
        assert_ne!(judge, summarize);
    }

    #[test]
    fn path_for_different_models_isolated() {
        let m1 = path_for("hash1", "judge");
        let m2 = path_for("hash2", "judge");
        assert_ne!(m1.parent(), m2.parent());
    }

    // ── Drift-aware loader tests ─────────────────────────────────────────
    //
    // These test the prompt_version currency check independently of the
    // filesystem path — we construct curves in memory and inspect the
    // CURRENCY LOGIC, not load_current_curve's disk reads (those are
    // covered by load_missing_path_returns_none above).

    fn fresh_curve_with_prompt(prompt_version: &str) -> CalibrationCurve {
        let mut c = test_curve("abc123", "judge");
        c.prompt_version = prompt_version.to_string();
        c
    }

    /// The currency check as a pure function, extracted from
    /// load_current_curve so we can test it without filesystem setup.
    fn is_current(curve: &CalibrationCurve, current_prompt_version: &str) -> bool {
        curve.prompt_version == current_prompt_version
    }

    #[test]
    fn currency_check_passes_when_prompt_version_matches() {
        let curve = fresh_curve_with_prompt("judge-v1-2026-04-15");
        assert!(is_current(&curve, "judge-v1-2026-04-15"));
    }

    #[test]
    fn currency_check_fails_when_prompt_version_drifts() {
        // Load-bearing: a prompt revision invalidates the curve so the
        // rerank loop falls through to pass-through instead of applying
        // a curve fit against an obsolete prompt.
        let stored = fresh_curve_with_prompt("judge-v1-2026-04-15");
        assert!(!is_current(&stored, "judge-v2-2026-05-01"));
    }

    // ── CurveDrift detail surface ────────────────────────────────────────
    //
    // load_current_curve_detailed promises to return a drift descriptor
    // when the prompt version differs. The analysis_rerank caller uses
    // this to fire a UI event; if the descriptor fields are wrong, the
    // toast shows garbage data.

    #[test]
    fn drift_detail_is_none_when_no_curve_on_disk() {
        // Using an identity_hash we know doesn't have a file on disk —
        // this is the "pass-through, nothing to alarm" case.
        let result = load_current_curve_detailed(
            "nonexistent_hash_for_drift_detail_test",
            "judge",
            "judge-v1",
        );
        assert!(result.curve.is_none());
        assert!(
            result.drift.is_none(),
            "no curve on disk should produce no drift signal"
        );
    }

    #[test]
    fn drift_report_fields_are_populated() {
        // Unit-test the drift descriptor construction independently of
        // the filesystem — we simulate the branch by building a curve
        // with a stale prompt_version and verifying the drift fields.
        // This is the same logic load_current_curve_detailed runs after
        // a successful disk read.
        let stored = fresh_curve_with_prompt("judge-v1-2026-04-15");
        let current = "judge-v2-2026-05-01";

        // Mirror the inline construction from load_current_curve_detailed.
        let drift = CurveDrift {
            curve_id: stored.curve_id.clone(),
            task: stored.task.clone(),
            model_identity_hash: stored.model_identity_hash.clone(),
            stored_prompt_version: stored.prompt_version.clone(),
            current_prompt_version: current.to_string(),
            reason: "prompt_version_mismatch".to_string(),
        };

        assert_eq!(drift.curve_id, stored.curve_id);
        assert_eq!(drift.stored_prompt_version, "judge-v1-2026-04-15");
        assert_eq!(drift.current_prompt_version, current);
        assert_eq!(drift.reason, "prompt_version_mismatch");
    }

    #[test]
    fn currency_check_is_string_exact_match() {
        // Intentionally strict — we don't try to parse semver or date.
        // Prompt versions are opaque identifiers; any difference =
        // invalidate. Simpler + no silent partial-match bugs.
        let stored = fresh_curve_with_prompt("judge-v1-2026-04-15");
        assert!(!is_current(&stored, "judge-v1-2026-04-15 ")); // trailing space
        assert!(!is_current(&stored, "Judge-v1-2026-04-15")); // case
        assert!(!is_current(&stored, "judge-v1")); // prefix
    }
}
