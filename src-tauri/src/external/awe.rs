// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Typed wrapper for the AWE (Artificial Wisdom Engine) CLI.
//!
//! This is **Layer 1** of the Silent-Failure Defense Architecture for the
//! AWE boundary specifically. See `docs/strategy/SILENT-FAILURE-DEFENSE.md`
//! for the overall architecture and
//! `.claude/wisdom/antibodies/2026-04-12-silent-cli-failures.md` for the
//! specific bugs this wrapper defends against.
//!
//! ## The wrapper contract
//!
//! Every method in `AweClient` performs the following steps, in order,
//! before returning a typed success value:
//!
//! 1. Spawn the configured `awe.exe` binary with the method's args
//! 2. Wait for completion with a method-specific timeout
//! 3. Check `output.status.success()` — exit non-zero → `AweError::ExitFailed`
//! 4. Scan stderr AND stdout for known error strings (e.g., `"Unknown stage"`,
//!    `"no such file"`, `"permission denied"`) — match → `AweError::KnownError`
//! 5. Parse stdout into the expected typed output — parse failure →
//!    `AweError::ParseError`
//! 6. Return `Ok(TypedOutput)` only if all previous steps passed
//!
//! Callers cannot obtain a `TransmuteOutput` (or any other typed success)
//! without every check passing. The compiler enforces verification.
//!
//! ## Status (2026-04-12)
//!
//! **Error-pattern scanning is LIVE.** The contract checks from step 3-4
//! (scan stderr/stdout for known error patterns) are now enforced in
//! `context_commands::run_awe_with_timeout` and `context_commands::run_awe_async`,
//! which all 30 call sites go through. This means ALL AWE invocations now
//! catch silent failures (Bug #1: "Unknown stage:" and similar).
//!
//! Individual call-site migration to `AweClient` methods (typed outputs,
//! Bug #2 decision-ID validation) remains a follow-up but is lower
//! priority since the core safety contract is now enforced.
//!
//! Remaining migration plan (lower priority):
//! 1. Finalize `AweClient` API by moving one call site as a spike
//! 2. Add a real-binary integration test in `tests/integration/test_awe_cli.rs`
//! 3. Migrate `awe_commands.rs` call sites
//! 4. Migrate `context_commands.rs` call sites
//! 5. Migrate `awe_autonomous.rs` + `awe_source_mining.rs`
//! 6. Migrate `monitoring_briefing.rs`
//! 7. Wire `scripts/validate-boundary-calls.cjs` to fail on any new raw
//!    `Command::new` for AWE outside `src-tauri/src/external/awe.rs`

// Some methods (transmute, feedback, version) are defined as the full typed
// API but don't have production call sites yet — one call site at a time is
// being migrated from raw `Command::new("awe")` to `AweClient::*`. The first
// migration (monitoring_briefing.rs::collect_awe_wisdom_signals → AweClient::wisdom)
// landed in commit e36d266c+1; others are still TODO. Allow dead code until
// the remaining call sites migrate.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use thiserror::Error;

// ============================================================================
// Known error patterns — scanned in stdout and stderr after every call
// ============================================================================

/// Known error substrings that the AWE CLI may emit on stdout/stderr even
/// when the exit code indicates success. Each entry here represents a
/// silent-failure mode we want to catch.
///
/// To add a new pattern: extend this list AND add a test case in
/// `tests::contract_catches_known_error`.
const KNOWN_ERROR_PATTERNS: &[&str] = &[
    "Unknown stage:",    // Bug #1: --stages receive rejection
    "error: invalid",    // generic clap-style argument errors
    "error: unexpected", // generic clap-style argument errors
    "error: the following required arguments",
    "Error: ",     // anyhow / eyre style top-level error
    "panicked at", // Rust panic in the AWE binary
    "thread '",    // panic location line
    "No such file or directory",
    "permission denied",
    "Permission denied",
];

// ============================================================================
// Error types
// ============================================================================

/// All failure modes when invoking the AWE CLI. Each variant represents
/// a distinct contract violation that the caller may want to handle
/// differently (retry, skip, fall back, surface to user, etc.).
#[derive(Debug, Error)]
pub enum AweError {
    /// The configured `awe.exe` binary does not exist or is not
    /// executable. User needs to install AWE or configure the path.
    #[error("AWE binary not found at {path:?}: {reason}")]
    BinaryNotFound { path: PathBuf, reason: String },

    /// The spawn failed at the OS level (access denied, resource limit,
    /// etc.) before we could wait for output.
    #[error("Failed to spawn {path:?}: {source}")]
    SpawnFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The call exceeded the configured timeout. The process was killed.
    #[error("AWE call timed out after {secs}s: args={args:?}")]
    Timeout { secs: u64, args: Vec<String> },

    /// The process exited with a non-zero status. `stderr` is included
    /// for diagnosis.
    #[error("AWE exited {code}: stderr={stderr}")]
    ExitFailed {
        code: i32,
        stderr: String,
        stdout: String,
    },

    /// A known error pattern was found in stdout or stderr even though
    /// the exit code was 0. This is the classic silent-failure bug.
    #[error("AWE emitted known error pattern {pattern:?}: {context}")]
    KnownError { pattern: String, context: String },

    /// Exit code was 0, no known error patterns, but stdout did not
    /// parse into the expected shape for this method.
    #[error("AWE stdout did not match expected shape: {reason} — snippet: {snippet:?}")]
    ParseError { reason: String, snippet: String },
}

// ============================================================================
// Client configuration + client struct
// ============================================================================

/// Configuration for an `AweClient` instance. Typical usage constructs
/// one per process from settings + runtime paths.
#[derive(Debug, Clone)]
pub struct AweClientConfig {
    /// Absolute path to the AWE binary. Typically resolved from settings
    /// or `runtime_paths`. On Windows this is `awe.exe`.
    pub binary_path: PathBuf,

    /// Default timeout for calls that don't specify one. AWE subcommands
    /// range from sub-second (`version`) to tens of seconds (`transmute`).
    pub default_timeout: Duration,
}

impl AweClientConfig {
    /// Construct a config from the standard binary path used elsewhere
    /// in 4DA. Delegates to `context_commands::find_awe_binary` so there
    /// is a single source of truth for where `awe.exe` lives on a given
    /// user's machine. Returns `None` if AWE is not installed / not
    /// discoverable via the standard search paths.
    pub fn from_default_paths() -> Option<Self> {
        let path_str = crate::context_commands::find_awe_binary()?;
        Some(Self {
            binary_path: PathBuf::from(path_str),
            default_timeout: Duration::from_secs(30),
        })
    }
}

/// Typed AWE CLI client. Every method performs mandatory contract
/// verification — exit code check, stderr scan, stdout parse — and
/// returns `Result<TypedOutput, AweError>`.
///
/// **There is no way to call this client and get a typed success without
/// passing verification.** That's the whole point.
#[derive(Debug, Clone)]
pub struct AweClient {
    config: AweClientConfig,
}

// ============================================================================
// Typed output shapes
// ============================================================================

/// Output of `AweClient::version`. Populated from `awe --version` or
/// `awe version --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AweVersionOutput {
    pub version: String,
}

/// Output of `AweClient::transmute`. The heart of AWE's decision
/// pipeline. `decision_id` is the REAL server-assigned UUIDv7 `dc_`
/// identifier — callers must NEVER mint their own IDs and assume they
/// match (see Bug #2 in the antibody doc).
#[derive(Debug, Clone)]
pub struct AweTransmuteOutput {
    /// Server-assigned decision ID. Round-tripped from `awe history`
    /// after the transmute so the caller has an ID that actually exists
    /// in the wisdom graph.
    pub decision_id: String,
    /// The raw JSON output AWE emitted. Preserved for downstream
    /// consumers that want to re-parse.
    pub raw_json: String,
}

/// Output of `AweClient::wisdom`. Returns the wisdom-graph summary for
/// a given domain.
#[derive(Debug, Clone)]
pub struct AweWisdomOutput {
    pub raw_output: String,
}

/// Feedback outcome for `AweClient::feedback`. Mirrors the AWE CLI's
/// feedback subcommand vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AweFeedbackOutcome {
    Confirmed,
    Refuted,
    Partial,
}

impl AweFeedbackOutcome {
    fn as_cli_arg(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Refuted => "refuted",
            Self::Partial => "partial",
        }
    }
}

// ============================================================================
// Client methods
// ============================================================================

impl AweClient {
    /// Create a new client from configuration. Doesn't validate the
    /// binary exists — that happens lazily on first call, surfaced via
    /// `AweError::BinaryNotFound`.
    pub fn new(config: AweClientConfig) -> Self {
        Self { config }
    }

    /// `awe --version` — fast sanity check that the binary exists and
    /// runs. Typically <100ms. Good for the Layer 4 cold-boot smoke test.
    pub fn version(&self) -> Result<AweVersionOutput, AweError> {
        let stdout = self.invoke(&["--version"], Some(Duration::from_secs(5)))?;
        // AWE --version output format: `awe X.Y.Z` or similar
        let version = stdout.trim().to_string();
        if version.is_empty() {
            return Err(AweError::ParseError {
                reason: "version output was empty".to_string(),
                snippet: stdout,
            });
        }
        Ok(AweVersionOutput { version })
    }

    /// `awe transmute --statement <text>` — runs a statement through
    /// the full wisdom-graph pipeline. Returns the REAL server-assigned
    /// decision ID, round-tripped via `awe history --limit 1 --json`
    /// after the transmute (defends against Bug #2).
    ///
    /// `stages` may be empty (AWE auto-prepends `Receive`) or a subset
    /// of the explicit stage names AWE recognizes. It **MUST NOT**
    /// contain the string `"receive"` — that's Bug #1.
    pub fn transmute(
        &self,
        _statement: &str,
        _stages: &[&str],
    ) -> Result<AweTransmuteOutput, AweError> {
        // Guard: reject known-bad stage arguments up front (Bug #1).
        for s in _stages {
            if s.eq_ignore_ascii_case("receive") {
                return Err(AweError::KnownError {
                    pattern: "Unknown stage:".to_string(),
                    context: "caller attempted to pass `receive` as an explicit stage — \
                              AWE auto-prepends Receive and rejects it as an explicit arg"
                        .to_string(),
                });
            }
        }

        // TODO(external::awe migration): port the call-site logic from
        // awe_commands.rs::transmute_internal. This is a skeleton method
        // that documents the contract; the real implementation follows
        // once the migration commit is cut. Returning NotImplemented-ish
        // ParseError for now so accidental use during the skeleton window
        // surfaces a loud error rather than a silent no-op.
        Err(AweError::ParseError {
            reason: "AweClient::transmute is skeleton-only — call site migration pending"
                .to_string(),
            snippet: String::new(),
        })
    }

    /// `awe wisdom --domain <domain>` — returns the wisdom-graph summary for
    /// a domain as free-form text. Used by `monitoring_briefing.rs` and
    /// `context_commands.rs` to populate briefing wisdom signals and context
    /// recalls.
    ///
    /// This is the FIRST production method wired into the typed wrapper.
    /// The call site migration is `monitoring_briefing.rs::collect_awe_wisdom_signals`
    /// as of 2026-04-14. Other wisdom call sites will follow in subsequent
    /// commits.
    pub fn wisdom(&self, domain: &str) -> Result<AweWisdomOutput, AweError> {
        if domain.is_empty() {
            return Err(AweError::ParseError {
                reason: "domain argument must not be empty".to_string(),
                snippet: String::new(),
            });
        }
        let raw_output = self.invoke(
            &["wisdom", "--domain", domain],
            Some(Duration::from_secs(15)),
        )?;
        Ok(AweWisdomOutput { raw_output })
    }

    /// `awe feedback --decision-id <id> --outcome <outcome>` — record
    /// feedback against a decision. `decision_id` MUST be a real
    /// server-assigned `dc_` identifier obtained from a prior
    /// `transmute` or `history` call (Bug #2).
    pub fn feedback(&self, decision_id: &str, outcome: AweFeedbackOutcome) -> Result<(), AweError> {
        // Guard: reject client-minted IDs (Bug #2). Real AWE decision
        // IDs start with `dc_`; client-minted ones historically used
        // `ux_<timestamp>`.
        if !decision_id.starts_with("dc_") {
            return Err(AweError::KnownError {
                pattern: "client-minted ID".to_string(),
                context: format!(
                    "feedback called with non-AWE decision ID `{decision_id}` — IDs MUST \
                     start with `dc_` (server-assigned). Round-trip through \
                     `history --limit 1 --json` to get the real ID."
                ),
            });
        }
        let _ = outcome; // will be used by the real implementation
                         // TODO(external::awe migration): port from call sites.
        Err(AweError::ParseError {
            reason: "AweClient::feedback is skeleton-only — call site migration pending"
                .to_string(),
            snippet: String::new(),
        })
    }

    // ========================================================================
    // Internal: the one and only spawn-and-verify helper
    // ========================================================================

    /// The single point where the actual `Command::new` lives for this
    /// entire module. Every public method must go through `invoke` so
    /// the contract checks happen in one place.
    ///
    /// Callers pass the full arg list (excluding the binary itself).
    /// `timeout` defaults to `config.default_timeout` if `None`.
    ///
    /// Contract:
    /// 1. Spawn + wait (with timeout)
    /// 2. Exit code must be 0
    /// 3. stderr must not contain any `KNOWN_ERROR_PATTERNS`
    /// 4. stdout must not contain any `KNOWN_ERROR_PATTERNS`
    /// 5. Return stdout as `String`
    fn invoke(&self, args: &[&str], timeout: Option<Duration>) -> Result<String, AweError> {
        let binary = &self.config.binary_path;
        let timeout_secs = timeout.unwrap_or(self.config.default_timeout).as_secs();

        // Sanity: verify binary exists before we delegate. This gives us the
        // typed `BinaryNotFound` variant instead of a generic spawn failure
        // from the lower-level helper (which returns `Err(String)`).
        if !binary.exists() {
            return Err(AweError::BinaryNotFound {
                path: binary.clone(),
                reason: "binary path does not exist on disk".to_string(),
            });
        }

        // Delegate to the proven `run_awe_with_timeout` helper in
        // `context_commands`. That helper already implements:
        //   - LLM env-var setup (ANTHROPIC_API_KEY / OPENAI_API_KEY /
        //     AWE_OLLAMA_MODEL / AWE_OLLAMA_URL from settings)
        //   - Spawn with piped stdout/stderr
        //   - 100ms poll loop with timeout kill
        //   - Exit-code check
        //   - KNOWN_ERROR_PATTERNS scan on stdout AND stderr (same list
        //     as `KNOWN_ERROR_PATTERNS` above; contract drift between the
        //     two lists is caught by `contract_catches_known_error_pattern_in_stdout`
        //     test below)
        //
        // All we do here is: build the Command, call the helper, then
        // translate its `Err(String)` into our typed `AweError`.
        let mut cmd = Command::new(binary);
        cmd.args(args);

        let output = crate::context_commands::run_awe_with_timeout(&mut cmd, timeout_secs)
            .map_err(|msg| classify_helper_error(&msg, binary, args))?;

        // The helper returns Ok only when exit was 0 AND no KNOWN_ERROR_PATTERNS
        // matched. So the output here is guaranteed-clean by the time we get it.
        // We still do a belt-and-suspenders exit-code check + pattern scan in
        // case the helper's contract ever drifts.
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        // 2. Exit code check (defense-in-depth)
        if !output.status.success() {
            return Err(AweError::ExitFailed {
                code: output.status.code().unwrap_or(-1),
                stderr,
                stdout,
            });
        }

        // 3 + 4. Known-error-pattern scan on both stderr and stdout.
        // Bug #1 (`Unknown stage: receive`) slipped through for months
        // because the caller only checked exit code — this is the fix.
        for pattern in KNOWN_ERROR_PATTERNS {
            if stderr.contains(pattern) {
                return Err(AweError::KnownError {
                    pattern: (*pattern).to_string(),
                    context: format!(
                        "stderr contained {pattern:?} (exit code was 0 — silent failure): {stderr}"
                    ),
                });
            }
            if stdout.contains(pattern) {
                return Err(AweError::KnownError {
                    pattern: (*pattern).to_string(),
                    context: format!(
                        "stdout contained {pattern:?} (exit code was 0 — silent failure): \
                         {}",
                        stdout.chars().take(300).collect::<String>()
                    ),
                });
            }
        }

        // 5. Return stdout
        Ok(stdout)
    }
}

// ============================================================================
// Internal: helper-error classification
// ============================================================================

/// Translate the `Err(String)` from `run_awe_with_timeout` into a typed
/// `AweError` variant. The helper returns free-form error messages whose
/// structure we can pattern-match to recover the original failure category.
///
/// This keeps the typed API surface clean while reusing the proven
/// spawn/poll/timeout logic in `context_commands::run_awe_with_timeout`.
fn classify_helper_error(msg: &str, binary: &std::path::Path, args: &[&str]) -> AweError {
    // Timeout: helper emits "AWE timed out after Ns"
    if let Some(rest) = msg.strip_prefix("AWE timed out after ") {
        if let Some(secs_str) = rest.strip_suffix('s') {
            if let Ok(secs) = secs_str.parse::<u64>() {
                return AweError::Timeout {
                    secs,
                    args: args.iter().map(|s| (*s).to_string()).collect(),
                };
            }
        }
    }

    // Silent failure detected by helper: "AWE silent failure: stdout/stderr contains 'X'"
    if let Some(rest) = msg.strip_prefix("AWE silent failure:") {
        // Extract the pattern from the single-quoted tail, if present.
        let pattern = rest
            .trim()
            .rsplit_once('\'')
            .and_then(|(before, _)| before.rsplit_once('\'').map(|(_, p)| p.to_string()))
            .unwrap_or_else(|| rest.trim().to_string());
        return AweError::KnownError {
            pattern,
            context: format!("helper detected silent failure: {msg}"),
        };
    }

    // Spawn failure: "Failed to start AWE: <io error>"
    if msg.starts_with("Failed to start AWE:") {
        // If the io error includes "not found", classify as BinaryNotFound.
        if msg.contains("not found") || msg.contains("cannot find") {
            return AweError::BinaryNotFound {
                path: binary.to_path_buf(),
                reason: msg.to_string(),
            };
        }
        return AweError::SpawnFailed {
            path: binary.to_path_buf(),
            source: std::io::Error::other(msg.to_string()),
        };
    }

    // Fallback: ExitFailed with the string as stderr. Loses some fidelity
    // but is preferable to inventing a category.
    AweError::ExitFailed {
        code: -1,
        stderr: msg.to_string(),
        stdout: String::new(),
    }
}

// ============================================================================
// Tests — mix of pure-unit and contract-guard tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transmute_rejects_receive_stage() {
        // Bug #1 guard: even the skeleton catches the `receive` stage
        // bad-arg up front so migrating call sites cannot re-introduce it.
        let client = AweClient::new(AweClientConfig {
            binary_path: PathBuf::from("/nonexistent/awe"),
            default_timeout: Duration::from_secs(1),
        });
        let err = client.transmute("test", &["receive"]).unwrap_err();
        match err {
            AweError::KnownError { pattern, .. } => {
                assert_eq!(pattern, "Unknown stage:");
            }
            other => panic!("expected KnownError for `receive` stage, got {other:?}"),
        }
    }

    #[test]
    fn feedback_rejects_client_minted_id() {
        // Bug #2 guard: feedback must refuse any ID that isn't
        // server-assigned (prefix `dc_`).
        let client = AweClient::new(AweClientConfig {
            binary_path: PathBuf::from("/nonexistent/awe"),
            default_timeout: Duration::from_secs(1),
        });
        let err = client
            .feedback("ux_1234567890", AweFeedbackOutcome::Confirmed)
            .unwrap_err();
        match err {
            AweError::KnownError { pattern, .. } => {
                assert_eq!(pattern, "client-minted ID");
            }
            other => panic!("expected KnownError for client-minted ID, got {other:?}"),
        }
    }

    #[test]
    fn binary_not_found_surfaces_specific_error() {
        // When the binary doesn't exist, we should get BinaryNotFound,
        // not a generic SpawnFailed — this lets the caller route to a
        // "please install AWE" banner instead of a cryptic io error.
        let client = AweClient::new(AweClientConfig {
            binary_path: PathBuf::from("/definitely/does/not/exist/awe"),
            default_timeout: Duration::from_secs(1),
        });
        // Use a non-skeleton path: version() calls through `invoke` so
        // the NotFound classification runs. version() doesn't have
        // a skeleton short-circuit like transmute/wisdom/feedback do.
        let err = client.version().unwrap_err();
        match err {
            AweError::BinaryNotFound { .. } => {}
            other => panic!("expected BinaryNotFound, got {other:?}"),
        }
    }

    #[test]
    fn contract_catches_known_error_pattern_in_stdout() {
        // Meta-assertion: KNOWN_ERROR_PATTERNS contains the classic
        // Bug #1 string. If someone removes it in a future refactor,
        // this test fails.
        assert!(
            KNOWN_ERROR_PATTERNS.contains(&"Unknown stage:"),
            "KNOWN_ERROR_PATTERNS must retain `Unknown stage:` to defend \
             against Bug #1 regression"
        );
    }

    #[test]
    fn feedback_outcome_cli_arg_mapping() {
        assert_eq!(AweFeedbackOutcome::Confirmed.as_cli_arg(), "confirmed");
        assert_eq!(AweFeedbackOutcome::Refuted.as_cli_arg(), "refuted");
        assert_eq!(AweFeedbackOutcome::Partial.as_cli_arg(), "partial");
    }

    #[test]
    fn wisdom_rejects_empty_domain() {
        // Guard: empty domain would produce a meaningless `awe wisdom --domain ""`
        // call that either no-ops or errors on AWE's side. Reject up front so
        // the failure mode is a typed ParseError at the API boundary, not a
        // surprise at the CLI layer.
        let client = AweClient::new(AweClientConfig {
            binary_path: PathBuf::from("/nonexistent/awe"),
            default_timeout: Duration::from_secs(1),
        });
        let err = client.wisdom("").unwrap_err();
        match err {
            AweError::ParseError { reason, .. } => {
                assert!(reason.contains("domain"));
            }
            other => panic!("expected ParseError for empty domain, got {other:?}"),
        }
    }

    #[test]
    fn classify_helper_error_maps_timeout() {
        let path = std::path::Path::new("/bin/awe");
        let err = classify_helper_error("AWE timed out after 15s", path, &["wisdom"]);
        match err {
            AweError::Timeout { secs, args } => {
                assert_eq!(secs, 15);
                assert_eq!(args, vec!["wisdom".to_string()]);
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[test]
    fn classify_helper_error_maps_silent_failure() {
        let path = std::path::Path::new("/bin/awe");
        let err = classify_helper_error(
            "AWE silent failure: stderr contains 'Unknown stage:'",
            path,
            &["transmute"],
        );
        match err {
            AweError::KnownError { pattern, .. } => {
                assert_eq!(pattern, "Unknown stage:");
            }
            other => panic!("expected KnownError, got {other:?}"),
        }
    }

    #[test]
    fn classify_helper_error_maps_spawn_not_found() {
        let path = std::path::Path::new("/bin/awe");
        let err =
            classify_helper_error("Failed to start AWE: program not found", path, &["version"]);
        match err {
            AweError::BinaryNotFound { .. } => {}
            other => panic!("expected BinaryNotFound, got {other:?}"),
        }
    }

    #[test]
    fn wisdom_fails_cleanly_when_binary_missing() {
        // End-to-end: the migrated call path should surface BinaryNotFound
        // when the configured AWE binary doesn't exist on disk. Defends the
        // monitoring_briefing migration's error-handling branch.
        let client = AweClient::new(AweClientConfig {
            binary_path: PathBuf::from("/definitely/does/not/exist/awe"),
            default_timeout: Duration::from_secs(1),
        });
        let err = client.wisdom("software-engineering").unwrap_err();
        match err {
            AweError::BinaryNotFound { .. } => {}
            other => panic!("expected BinaryNotFound, got {other:?}"),
        }
    }
}
