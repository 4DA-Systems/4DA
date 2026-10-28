// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The host's files, as the lockfile readers see them.
//!
//! The readers never touch a file system. Everything they need beyond the
//! lockfile text itself comes through [`FileSource`]:
//! - which dependency files a directory holds (`is_file` per fixed name);
//! - the files a directory lists (`.csproj` / `.fsproj` / `.vbproj`
//!   projects, `*requirements*.txt`, `requirements/*.txt`);
//! - a lockfile's companions: `go.sum` beside a pre-1.17 `go.mod`, and the
//!   `-r` / `--requirement` includes of a requirements file.
//!
//! Paths are host strings. The host also joins them ([`FileSource::join`],
//! [`FileSource::parent`]), so an outcome names a file exactly as the host
//! spells it (`D:\app\package-lock.json` on Windows). [`MemFs`] is a host
//! made of a map, for tests and for a wasm caller that ships the files in
//! the request.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry's own name (no directory part).
    pub name: String,
    /// A regular file (a symlink to one counts).
    pub is_file: bool,
}

/// What a host lets the readers see.
pub trait FileSource {
    /// The file's text. `Err` carries why it could not be read; the outcome
    /// says `unreadable: <err>`.
    fn read(&self, path: &str) -> Result<String, String>;

    /// Whether `path` names a regular file (a symlink to one counts).
    fn is_file(&self, path: &str) -> bool;

    /// The entries directly in `dir`, in any order. `None` when `dir` is not
    /// a directory the host can list.
    fn list(&self, dir: &str) -> Option<Vec<Entry>>;

    /// `dir` joined with `name` in the host's spelling. The default joins
    /// with `/`; a name that is already absolute replaces `dir`.
    fn join(&self, dir: &str, name: &str) -> String {
        if dir.is_empty() || name.starts_with('/') {
            name.to_string()
        } else if dir.ends_with(['/', '\\']) {
            format!("{dir}{name}")
        } else {
            format!("{dir}/{name}")
        }
    }

    /// The directory holding `path` (`""` for a bare file name), `None` for
    /// a root or an empty path.
    fn parent(&self, path: &str) -> Option<String> {
        let trimmed = path.trim_end_matches(['/', '\\']);
        if trimmed.is_empty() {
            return None;
        }
        Some(match trimmed.rfind(['/', '\\']) {
            Some(0) => trimmed[..1].to_string(),
            Some(at) => trimmed[..at].to_string(),
            None => String::new(),
        })
    }
}

/// A path's components: separators (`/` and `\`) and `.` dropped, `..`
/// kept. Two spellings of one path (`a/./b.txt`, `a//b.txt`) agree here, as
/// they do for `std::path::Path`'s comparison.
pub fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != ".")
        .collect()
}

/// Paths ordered component by component (`std::path::Path`'s order), so
/// `requirements/base.txt` sorts before `requirements-dev.txt`.
pub fn cmp_paths(a: &str, b: &str) -> Ordering {
    components(a).cmp(&components(b))
}

/// The last component of a path; `None` for an empty path or `..`.
pub fn file_name(path: &str) -> Option<&str> {
    components(path).last().copied().filter(|n| *n != "..")
}

/// A file name's extension (`"txt"` for `a.b.txt`); `None` when it has no
/// dot or is a dotfile (`.txt`), as `std::path::Path::extension` reads it.
pub fn extension(name: &str) -> Option<&str> {
    if name == ".." {
        return None;
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => Some(ext),
        _ => None,
    }
}

/// A file system held in memory: `path -> text`, directories implied by the
/// paths. Paths are compared by [`components`], so `/p/a.txt` and
/// `/p/./a.txt` name one file.
#[derive(Debug, Clone, Default)]
pub struct MemFs {
    files: BTreeMap<Vec<String>, (String, String)>,
}

impl MemFs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) a file.
    #[must_use]
    pub fn with(mut self, path: &str, text: &str) -> Self {
        self.insert(path, text);
        self
    }

    /// Add (or replace) a file.
    pub fn insert(&mut self, path: &str, text: &str) {
        self.files
            .insert(key(path), (path.to_string(), text.to_string()));
    }
}

fn key(path: &str) -> Vec<String> {
    components(path).into_iter().map(str::to_string).collect()
}

impl FileSource for MemFs {
    fn read(&self, path: &str) -> Result<String, String> {
        self.files
            .get(&key(path))
            .map(|(_, text)| text.clone())
            .ok_or_else(|| "not found".to_string())
    }

    fn is_file(&self, path: &str) -> bool {
        self.files.contains_key(&key(path))
    }

    fn list(&self, dir: &str) -> Option<Vec<Entry>> {
        let dir = key(dir);
        let mut out: BTreeMap<String, bool> = BTreeMap::new();
        for path in self.files.keys() {
            if path.len() > dir.len() && path.starts_with(&dir) {
                let is_file = path.len() == dir.len() + 1;
                let name = path[dir.len()].clone();
                let was_file = out.get(&name).copied().unwrap_or(false);
                out.insert(name, is_file || was_file);
            }
        }
        if out.is_empty() {
            return None;
        }
        Some(
            out.into_iter()
                .map(|(name, is_file)| Entry { name, is_file })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_join_and_parent_read_both_separators() {
        let fs = MemFs::new();
        assert_eq!(fs.join("/p", "a.txt"), "/p/a.txt");
        assert_eq!(fs.join("/p/", "a.txt"), "/p/a.txt");
        assert_eq!(fs.join("", "a.txt"), "a.txt");
        assert_eq!(fs.join("/p", "/abs.txt"), "/abs.txt");
        assert_eq!(fs.parent("/p/go.mod").as_deref(), Some("/p"));
        assert_eq!(fs.parent("C:\\p\\go.mod").as_deref(), Some("C:\\p"));
        assert_eq!(fs.parent("go.mod").as_deref(), Some(""));
        assert_eq!(fs.parent("/go.mod").as_deref(), Some("/"));
        assert_eq!(fs.parent(""), None);
    }

    #[test]
    fn paths_compare_and_name_like_std_path() {
        assert_eq!(
            cmp_paths("/p/requirements/base.txt", "/p/requirements-dev.txt"),
            Ordering::Less
        );
        assert_eq!(cmp_paths("/p/./a.txt", "/p//a.txt"), Ordering::Equal);
        assert_eq!(file_name("/p/requirements/"), Some("requirements"));
        assert_eq!(file_name("/p/.."), None);
        assert_eq!(extension("a.b.txt"), Some("txt"));
        assert_eq!(extension(".txt"), None);
        assert_eq!(extension("..txt"), Some("txt"));
        assert_eq!(extension("README"), None);
    }

    #[test]
    fn mem_fs_lists_files_and_implied_directories() {
        let fs = MemFs::new()
            .with("/p/requirements.txt", "x==1\n")
            .with("/p/requirements/base.txt", "y==2\n");
        let mut listed = fs.list("/p").unwrap_or_default();
        listed.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            listed,
            [
                Entry {
                    name: "requirements".into(),
                    is_file: false
                },
                Entry {
                    name: "requirements.txt".into(),
                    is_file: true
                },
            ]
        );
        assert!(fs.is_file("/p/./requirements.txt"));
        assert!(!fs.is_file("/p/requirements"));
        assert!(fs.list("/p/requirements.txt").is_none());
        assert_eq!(fs.read("/p/missing").unwrap_err(), "not found");
    }
}
