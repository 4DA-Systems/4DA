// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Egress boundary for LLM-bound text: the user's own filesystem paths do not
//! leave the machine.
//!
//! NETWORK.md promises that personal information never leaves the machine.
//! Measured 2026-09-10: all ten stored briefs contained `d:/4da/...` project
//! paths — proof they were in the synthesis prompt — because the brief's
//! CONFIRMED SECURITY builder named each affected project by its absolute
//! path. The same builder writes `c:/users/<username>/documents/navcal` the
//! day an advisory touches that project. `scripts/check-privacy-egress.cjs`
//! could not see it: it guards raw-content COLUMNS, and a path is not one.
//!
//! Fixing each builder is necessary (they now pass [`project_label`]s) but not
//! sufficient: the next builder will not know. So every prompt is rewritten
//! here, at the `LLMClient` entry points every provider call passes through
//! (`complete`, `complete_structured`, `complete_for_translation`,
//! `stream_complete` — a test below fails if an entry point dispatches to a
//! provider without calling [`scrub_prompt`]). Each occurrence of one of the
//! user's own local roots — a detected project, a configured context
//! directory, the home directory — becomes that root's short label. Only the
//! user's own roots are touched, so article text that happens to contain
//! `C:\Windows` passes through unchanged.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use crate::llm::Message;

/// How long a root snapshot is reused before it is rebuilt.
#[cfg(not(test))]
const ROOTS_TTL: std::time::Duration = std::time::Duration::from_mins(5);

/// One local directory whose occurrences are rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalRoot {
    /// Path segments, ASCII-lowercased; a Windows drive (`c:`) comes first.
    segments: Vec<String>,
    /// A Unix-absolute root (`/home/x`) must begin at a separator.
    unix_absolute: bool,
    /// What each occurrence is replaced with.
    label: String,
}

impl LocalRoot {
    fn specificity(&self) -> usize {
        self.segments.iter().map(String::len).sum::<usize>() + self.segments.len()
    }
}

fn is_sep(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

/// A byte that continues a path segment. Non-ASCII bytes count, so a root never
/// matches inside a longer non-ASCII directory name.
fn is_segment_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.') || b >= 0x80
}

fn is_drive(segment: &str) -> bool {
    let b = segment.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn raw_segments(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|s| !s.is_empty()).collect()
}

fn lowered_segments(path: &str) -> Vec<String> {
    raw_segments(path)
        .into_iter()
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The home directory's lowered segments, if the platform reports one.
fn home_segments() -> Option<Vec<String>> {
    static HOME: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        dirs::home_dir()
            .map(|p| lowered_segments(&p.to_string_lossy()))
            .filter(|s| !s.is_empty())
    })
    .clone()
}

/// Short, stable label for a local path: its last two segments, never the
/// home directory's name and never a drive letter.
///
/// `c:/users/<name>/documents/navcal` → `documents/navcal`;
/// `c:/users/<name>/navcal` → `~/navcal`; `d:/4da/relay` → `4da/relay`;
/// the home directory itself → `~`.
pub(crate) fn project_label(path: &str) -> String {
    label_with_home(path, home_segments().as_deref())
}

fn label_with_home(path: &str, home: Option<&[String]>) -> String {
    let raw = raw_segments(path);
    let lowered: Vec<String> = raw.iter().map(|s| s.to_ascii_lowercase()).collect();
    if let Some(home) = home {
        if !home.is_empty() && lowered.len() >= home.len() && lowered[..home.len()] == *home {
            let rest = &raw[home.len()..];
            return match rest.len() {
                0 => "~".to_string(),
                1 => format!("~/{}", rest[0]),
                n => format!("{}/{}", rest[n - 2], rest[n - 1]),
            };
        }
    }
    let body: &[&str] = match raw.first() {
        Some(first) if is_drive(first) => &raw[1..],
        _ => &raw[..],
    };
    match body.len() {
        0 => path.to_string(),
        1 => body[0].to_string(),
        n => format!("{}/{}", body[n - 2], body[n - 1]),
    }
}

/// A root for `path`, or `None` when the path is not specific enough to
/// rewrite safely: it must be absolute (a drive or a leading `/`) and have at
/// least two segments, so `C:\` or `/home` never rewrites unrelated text.
pub(crate) fn root_for(path: &str, label: String) -> Option<LocalRoot> {
    let segments = lowered_segments(path);
    let unix_absolute = path.starts_with('/');
    let absolute = unix_absolute || segments.first().is_some_and(|s| is_drive(s));
    if !absolute || segments.len() < 2 {
        return None;
    }
    Some(LocalRoot {
        segments,
        unix_absolute,
        label,
    })
}

/// Bytes of `text` matched by `root` starting at `pos`, or `None`. Runs of
/// either separator match one separator (JSON-escaped `C:\\Users\\x` included);
/// segments compare ASCII-case-insensitively; the match must end at a segment
/// boundary so `d:/4da` never matches inside `d:/4da-backup`.
fn match_len(bytes: &[u8], pos: usize, root: &LocalRoot) -> Option<usize> {
    let mut i = pos;
    if root.unix_absolute {
        if i >= bytes.len() || !is_sep(bytes[i]) {
            return None;
        }
        while i < bytes.len() && is_sep(bytes[i]) {
            i += 1;
        }
    }
    for (k, segment) in root.segments.iter().enumerate() {
        if k > 0 {
            let run_start = i;
            while i < bytes.len() && is_sep(bytes[i]) {
                i += 1;
            }
            if i == run_start {
                return None;
            }
        }
        let s = segment.as_bytes();
        let end = i.checked_add(s.len())?;
        if end > bytes.len() || !bytes[i..end].eq_ignore_ascii_case(s) {
            return None;
        }
        i = end;
    }
    if i < bytes.len() && is_segment_byte(bytes[i]) {
        return None;
    }
    Some(i - pos)
}

/// Byte length of the UTF-8 sequence that starts with `b`.
fn utf8_len(b: u8) -> usize {
    match b {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

/// Rewrite every occurrence of `roots` in `text` with its label. `roots` must
/// be ordered most-specific first ([`collect_roots`] does this) so a project
/// root beats the home directory that contains it.
pub(crate) fn scrub_with<'a>(text: &'a str, roots: &[LocalRoot]) -> Cow<'a, str> {
    if roots.is_empty() || text.is_empty() {
        return Cow::Borrowed(text);
    }
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let mut copied = 0usize;
    let mut pos = 0usize;
    while pos < bytes.len() {
        let b = bytes[pos];
        let may_start = (is_sep(b)
            || (b.is_ascii_alphabetic() && bytes.get(pos + 1) == Some(&b':')))
            && (pos == 0 || !is_segment_byte(bytes[pos - 1]));
        if may_start {
            if let Some((len, label)) = roots
                .iter()
                .find_map(|r| match_len(bytes, pos, r).map(|len| (len, r.label.as_str())))
            {
                let buf = out.get_or_insert_with(|| String::with_capacity(text.len()));
                buf.push_str(&text[copied..pos]);
                buf.push_str(label);
                pos += len;
                copied = pos;
                continue;
            }
        }
        pos += utf8_len(b);
    }
    match out {
        None => Cow::Borrowed(text),
        Some(mut buf) => {
            buf.push_str(&text[copied..]);
            Cow::Owned(buf)
        }
    }
}

/// Order roots most-specific first and drop duplicates.
fn order_roots(roots: Vec<LocalRoot>) -> Vec<LocalRoot> {
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    let mut unique: Vec<LocalRoot> = roots
        .into_iter()
        .filter(|r| seen.insert(r.segments.clone()))
        .collect();
    unique.sort_by_key(|r| std::cmp::Reverse(r.specificity()));
    unique
}

/// Every local root the user owns: detected projects, configured context
/// directories, and the home directory (labelled `~`).
#[cfg(not(test))]
fn collect_roots() -> Vec<LocalRoot> {
    let home = home_segments();
    let mut paths: Vec<String> = Vec::new();
    if let Ok(conn) = crate::open_db_connection() {
        if let Ok(mut stmt) = conn.prepare("SELECT path FROM detected_projects") {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                paths.extend(rows.flatten());
            }
        }
    }
    // Never wait on the settings lock from inside an LLM call: a caller that
    // holds it would deadlock. A skipped refresh still has the DB roots and
    // the home directory, and the next refresh retries.
    if let Some(sm) =
        crate::get_settings_manager().try_lock_for(std::time::Duration::from_millis(50))
    {
        paths.extend(sm.get().context_dirs.iter().cloned());
    }
    let mut roots: Vec<LocalRoot> = paths
        .iter()
        .filter_map(|p| root_for(p, label_with_home(p, home.as_deref())))
        .collect();
    if let Some(home_dir) = dirs::home_dir() {
        if let Some(r) = root_for(&home_dir.to_string_lossy(), "~".to_string()) {
            roots.push(r);
        }
    }
    order_roots(roots)
}

/// The current root set, rebuilt at most every [`ROOTS_TTL`].
#[cfg(not(test))]
fn local_roots() -> Arc<Vec<LocalRoot>> {
    use std::time::Instant;
    static CACHE: parking_lot::Mutex<Option<(Instant, Arc<Vec<LocalRoot>>)>> =
        parking_lot::Mutex::new(None);
    if let Some((at, roots)) = CACHE.lock().as_ref() {
        if at.elapsed() < ROOTS_TTL {
            return Arc::clone(roots);
        }
    }
    let roots = Arc::new(collect_roots());
    *CACHE.lock() = Some((Instant::now(), Arc::clone(&roots)));
    roots
}

/// Tests never read the real machine's roots; they drive [`scrub_with`].
#[cfg(test)]
fn local_roots() -> Arc<Vec<LocalRoot>> {
    Arc::new(Vec::new())
}

/// Rewrite the user's local paths in a prompt's system text and every
/// message. Called by every `LLMClient` entry point before dispatch.
pub(crate) fn scrub_prompt(system: &str, messages: Vec<Message>) -> (String, Vec<Message>) {
    let roots = local_roots();
    let system = scrub_with(system, &roots).into_owned();
    let messages = messages
        .into_iter()
        .map(|mut m| {
            let rewritten = match scrub_with(&m.content, &roots) {
                Cow::Borrowed(_) => None,
                Cow::Owned(s) => Some(s),
            };
            if let Some(s) = rewritten {
                m.content = s;
            }
            m
        })
        .collect();
    (system, messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> Vec<String> {
        vec!["c:".into(), "users".into(), "someone".into()]
    }

    fn roots() -> Vec<LocalRoot> {
        let h = home();
        order_roots(vec![
            root_for(
                r"C:\Users\Someone\Documents\navcal",
                label_with_home(r"C:\Users\Someone\Documents\navcal", Some(&h)),
            )
            .expect("navcal root"),
            root_for(
                "D:\\4DA\\relay",
                label_with_home("D:\\4DA\\relay", Some(&h)),
            )
            .expect("relay root"),
            root_for("D:\\4DA", label_with_home("D:\\4DA", Some(&h))).expect("4da root"),
            root_for(r"C:\Users\Someone", "~".to_string()).expect("home root"),
            root_for("/home/alice/code/app", "code/app".to_string()).expect("unix root"),
        ])
    }

    #[test]
    fn labels_never_carry_the_home_name_or_a_drive() {
        let h = home();
        let label = |p: &str| label_with_home(p, Some(&h));
        assert_eq!(
            label("c:/users/someone/documents/navcal"),
            "documents/navcal"
        );
        assert_eq!(label(r"C:\Users\Someone\navcal"), "~/navcal");
        assert_eq!(label(r"C:\Users\Someone"), "~");
        assert_eq!(label("d:/4da/relay"), "4da/relay");
        assert_eq!(label("D:\\4DA"), "4DA");
        assert_eq!(label("/srv/app"), "srv/app");
    }

    #[test]
    fn the_briefs_real_line_loses_its_paths() {
        // The CONFIRMED SECURITY line shape, with both path forms the DB holds.
        let line = "  - [HIGH] jsonwebtoken (9.3.1 -> update to >= 10.3.0): auth bypass \
                    -- affects: d:/4da/relay, c:/users/someone/documents/navcal (inactive 296 days)";
        let out = scrub_with(line, &roots());
        assert!(
            out.contains("affects: 4DA/relay, Documents/navcal (inactive 296 days)"),
            "{out}"
        );
        assert!(!out.to_lowercase().contains("c:/users"), "{out}");
        assert!(!out.contains("someone"), "{out}");
        assert!(!out.contains("d:/"), "{out}");
    }

    #[test]
    fn separators_case_and_json_escaping_all_match() {
        let r = roots();
        for text in [
            r"C:\Users\Someone\Documents\navcal\src\app.ts",
            "c:/USERS/someone/documents/NAVCAL/src/app.ts",
            r"C:\\Users\\Someone\\Documents\\navcal\\src\\app.ts",
        ] {
            let out = scrub_with(text, &r);
            assert!(out.starts_with("Documents/navcal"), "{text} -> {out}");
            assert!(!out.to_lowercase().contains("someone"), "{text} -> {out}");
        }
    }

    #[test]
    fn a_path_under_home_but_outside_any_project_keeps_only_the_tilde() {
        let out = scrub_with(r"see C:\Users\Someone\Downloads\x.log", &roots());
        assert_eq!(out, r"see ~\Downloads\x.log");
    }

    #[test]
    fn roots_match_only_at_segment_boundaries() {
        let r = roots();
        assert_eq!(scrub_with("d:/4da-backup/file", &r), "d:/4da-backup/file");
        assert_eq!(
            scrub_with("/home/alice/code/application", &r),
            "/home/alice/code/application"
        );
        assert_eq!(scrub_with("xd:/4da/relay", &r), "xd:/4da/relay");
        assert_eq!(scrub_with("/home/alice/code/app/src", &r), "code/app/src");
    }

    #[test]
    fn text_that_names_no_user_root_is_untouched_and_borrowed() {
        let text = r"Windows keeps it in C:\Windows\System32 → ✓ ünïcödé";
        assert!(matches!(scrub_with(text, &roots()), Cow::Borrowed(_)));
    }

    #[test]
    fn non_ascii_around_a_match_survives() {
        let out = scrub_with("→ d:/4da/relay ✓", &roots());
        assert_eq!(out, "→ 4DA/relay ✓");
    }

    #[test]
    fn roots_that_are_not_specific_are_refused() {
        assert!(root_for("C:\\", "x".into()).is_none());
        assert!(root_for("/home", "x".into()).is_none());
        assert!(root_for("relative/path", "x".into()).is_none());
    }

    /// The structural guarantee: every `LLMClient` entry point that dispatches
    /// to a provider calls `scrub_prompt` first. A new entry point that skips
    /// it fails here, whoever writes it.
    #[test]
    fn every_llm_entry_point_scrubs_before_dispatch() {
        let src = include_str!("llm.rs");
        let dispatch = [
            "self.complete_anthropic",
            "self.complete_openai",
            "self.complete_ollama",
            "llm_stream::stream_",
        ];
        let terminators = [
            "\n    pub ",
            "\n    async fn ",
            "\n    fn ",
            "\n    #[",
            "\n    ///",
        ];
        let mut checked = Vec::new();
        for (start, _) in src.match_indices("async fn ") {
            let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
            if !src[line_start..start].trim_start().starts_with("pub") {
                continue; // private provider functions are what entry points call
            }
            let rest = &src[start + 1..];
            let end = terminators
                .iter()
                .filter_map(|t| rest.find(t))
                .min()
                .map_or(src.len(), |i| start + 1 + i);
            let body = &src[start..end];
            if dispatch.iter().any(|d| body.contains(d)) {
                let name: String = body["async fn ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                assert!(
                    body.contains("privacy_egress::scrub_prompt"),
                    "LLMClient::{name} dispatches to a provider without privacy_egress::scrub_prompt"
                );
                checked.push(name);
            }
        }
        for expected in [
            "complete",
            "complete_structured",
            "complete_for_translation",
            "stream_complete",
        ] {
            assert!(
                checked.iter().any(|n| n == expected),
                "entry point {expected} not found by the guard (found {checked:?}) — update the guard"
            );
        }
    }
}
