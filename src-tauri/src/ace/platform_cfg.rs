// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Target/platform relevance for dependency advisories.
//!
//! Cargo gates dependencies behind a target, e.g.
//! `[target.'cfg(windows)'.dependencies]` or
//! `[target.x86_64-pc-windows-msvc.dependencies]`. A dependency that is only
//! active on a platform the user does not build is not relevant to them, so its
//! advisory would be noise (the classic "GTK/webkit vuln shown to a Windows
//! user"). This evaluates a target spec against the HOST so the scanner can mark
//! each dependency `platform_active`.
//!
//! Conservative by design: anything that cannot be confidently evaluated is
//! treated as ACTIVE, so a real advisory is never hidden on a guess.
//!
//! Mirrors `mcp-4da-server/src/live/platform.ts` so the app and the MCP server
//! agree on relevance.

/// Host target facts. `os`/`arch` are rustc-style; on a natively-compiled binary
/// these compile-time constants equal the machine 4DA is running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTarget {
    pub os: &'static str,
    pub family: &'static str,
    pub arch: &'static str,
}

/// The host the running binary targets (== the user's machine for a native build).
pub fn host_target() -> HostTarget {
    let os = std::env::consts::OS; // "windows" | "macos" | "linux" | ...
    let family = if os == "windows" { "windows" } else { "unix" };
    HostTarget {
        os,
        family,
        arch: std::env::consts::ARCH, // "x86_64" | "aarch64" | ...
    }
}

/// Is a dependency's target spec active on the host? `None`/empty means the dep
/// is unconditional (always active). Unknown specs resolve to active.
pub fn target_active_on_host(target: Option<&str>, host: HostTarget) -> bool {
    let spec = match target {
        Some(s) => s.trim(),
        None => return true,
    };
    if spec.is_empty() {
        return true;
    }
    if !spec.starts_with("cfg(") {
        // Explicit target triple, e.g. "x86_64-pc-windows-msvc".
        return triple_active_on_host(spec, host);
    }
    // cfg(...) predicate — strip the outer `cfg(` and trailing `)`.
    let end = spec.rfind(')').unwrap_or(spec.len());
    eval_cfg(&spec[4..end], host)
}

/// Evaluate a cfg predicate body (already unwrapped from `cfg( ... )`).
pub fn eval_cfg(expr: &str, host: HostTarget) -> bool {
    let e = expr.trim();
    if e.is_empty() {
        return true;
    }
    if let Some(body) = match_call(e, "not") {
        return !eval_cfg(body, host);
    }
    if let Some(body) = match_call(e, "all") {
        return split_args(body).iter().all(|a| eval_cfg(a, host));
    }
    if let Some(body) = match_call(e, "any") {
        return split_args(body).iter().any(|a| eval_cfg(a, host));
    }
    if let Some((key, value)) = parse_kv(e) {
        return eval_predicate(key, value, host);
    }
    if e == "windows" {
        return host.family == "windows";
    }
    if e == "unix" {
        return host.family == "unix";
    }
    // Unknown predicate -> conservatively active (never hide a real advisory).
    true
}

fn eval_predicate(key: &str, value: &str, host: HostTarget) -> bool {
    match key {
        "target_os" => value == host.os,
        "target_family" => value == host.family,
        "target_arch" => value == host.arch,
        "windows" => host.family == "windows",
        "unix" => host.family == "unix",
        _ => true, // unknown key -> conservatively active
    }
}

/// Match `name( ...body... )` and return the body, or None if `expr` is not that call.
fn match_call<'a>(expr: &'a str, name: &str) -> Option<&'a str> {
    let rest = expr.strip_prefix(name)?.trim_start();
    if rest.starts_with('(') && rest.ends_with(')') {
        Some(&rest[1..rest.len() - 1])
    } else {
        None
    }
}

/// Parse a `key = "value"` predicate where key is a lowercase/underscore identifier.
fn parse_kv(e: &str) -> Option<(&str, &str)> {
    let (k, rest) = e.split_once('=')?;
    let k = k.trim();
    let rest = rest.trim();
    if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        let v = &rest[1..rest.len() - 1];
        if !k.is_empty() && k.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Some((k, v));
        }
    }
    None
}

/// Split top-level comma-separated args, respecting nested parentheses.
fn split_args(body: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0usize;
    for (i, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let a = body[start..i].trim();
                if !a.is_empty() {
                    args.push(a);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let a = body[start..].trim();
    if !a.is_empty() {
        args.push(a);
    }
    args
}

/// Evaluate an explicit target triple (e.g. x86_64-pc-windows-msvc) against the host.
fn triple_active_on_host(triple: &str, host: HostTarget) -> bool {
    let t = triple.to_lowercase();
    let triple_os = if t.contains("windows") {
        Some("windows")
    } else if t.contains("darwin") || t.contains("apple") {
        Some("macos")
    } else if t.contains("linux") {
        Some("linux")
    } else {
        None
    };
    if let Some(os) = triple_os {
        if os != host.os {
            return false;
        }
    }
    let triple_arch = if t.starts_with("x86_64") {
        Some("x86_64")
    } else if t.starts_with("aarch64") {
        Some("aarch64")
    } else if t.starts_with("i686") || t.starts_with("i586") {
        Some("x86")
    } else {
        None
    };
    if let Some(arch) = triple_arch {
        if arch != host.arch {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: HostTarget = HostTarget {
        os: "windows",
        family: "windows",
        arch: "x86_64",
    };
    const LINUX_ARM: HostTarget = HostTarget {
        os: "linux",
        family: "unix",
        arch: "aarch64",
    };

    #[test]
    fn unconditional_is_active() {
        assert!(target_active_on_host(None, WIN));
        assert!(target_active_on_host(Some(""), WIN));
    }

    #[test]
    fn bare_windows_unix() {
        assert!(target_active_on_host(Some("cfg(windows)"), WIN));
        assert!(!target_active_on_host(Some("cfg(windows)"), LINUX_ARM));
        assert!(!target_active_on_host(Some("cfg(unix)"), WIN));
        assert!(target_active_on_host(Some("cfg(unix)"), LINUX_ARM));
    }

    #[test]
    fn not_predicate() {
        assert!(!target_active_on_host(Some("cfg(not(windows))"), WIN));
        assert!(target_active_on_host(Some("cfg(not(windows))"), LINUX_ARM));
    }

    #[test]
    fn kv_predicates() {
        assert!(target_active_on_host(
            Some("cfg(target_os = \"windows\")"),
            WIN
        ));
        assert!(!target_active_on_host(
            Some("cfg(target_os = \"linux\")"),
            WIN
        ));
        assert!(target_active_on_host(
            Some("cfg(target_family = \"unix\")"),
            LINUX_ARM
        ));
        assert!(target_active_on_host(
            Some("cfg(target_arch = \"aarch64\")"),
            LINUX_ARM
        ));
        assert!(!target_active_on_host(
            Some("cfg(target_arch = \"x86_64\")"),
            LINUX_ARM
        ));
    }

    #[test]
    fn all_and_any() {
        assert!(target_active_on_host(
            Some("cfg(all(windows, target_arch = \"x86_64\"))"),
            WIN
        ));
        assert!(!target_active_on_host(
            Some("cfg(all(windows, target_arch = \"aarch64\"))"),
            WIN
        ));
        assert!(target_active_on_host(Some("cfg(any(unix, windows))"), WIN));
        assert!(!target_active_on_host(
            Some("cfg(any(target_os = \"macos\", target_os = \"ios\"))"),
            WIN
        ));
    }

    #[test]
    fn explicit_triples() {
        assert!(target_active_on_host(Some("x86_64-pc-windows-msvc"), WIN));
        assert!(!target_active_on_host(
            Some("x86_64-unknown-linux-gnu"),
            WIN
        ));
        assert!(!target_active_on_host(
            Some("aarch64-apple-darwin"),
            LINUX_ARM
        ));
        assert!(target_active_on_host(
            Some("aarch64-unknown-linux-gnu"),
            LINUX_ARM
        ));
    }

    #[test]
    fn unknown_is_conservatively_active() {
        assert!(eval_cfg("feature = \"some_feature\"", WIN));
        assert!(eval_cfg("mystery_predicate", WIN));
    }

    #[test]
    fn host_target_is_sane() {
        let h = host_target();
        assert!(!h.os.is_empty());
        assert!(h.family == "windows" || h.family == "unix");
    }
}
