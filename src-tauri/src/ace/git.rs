//! Git Analyzer - Extract context from git history
//!
//! Analyzes git repositories to understand:
//! - Recent commit activity
//! - Active branches
//! - File change patterns
//! - Topic extraction from commit messages

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Git analyzer configuration
#[derive(Debug, Clone)]
pub struct GitConfig {
    /// Maximum commits to analyze
    pub max_commits: usize,
    /// Days of history to consider
    pub history_days: u32,
    /// Include merge commits
    pub include_merges: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            max_commits: 100,
            history_days: 30,
            include_merges: false,
        }
    }
}

/// Signal from analyzing a git repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSignal {
    pub repo_path: PathBuf,
    pub repo_name: String,
    pub recent_commits: Vec<CommitInfo>,
    pub active_branches: Vec<BranchInfo>,
    pub file_activity: HashMap<String, u32>,
    pub extracted_topics: Vec<String>,
    pub commit_frequency: f32,
    pub last_commit: Option<String>,
    pub confidence: f32,
}

/// Information about a commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: String,
    pub files_changed: Vec<String>,
    pub insertions: u32,
    pub deletions: u32,
    pub topics: Vec<String>,
}

/// Information about a branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub last_commit: String,
    pub ahead: u32,
    pub behind: u32,
}

/// Git analyzer
pub struct GitAnalyzer {
    config: GitConfig,
}

impl GitAnalyzer {
    pub fn new(config: GitConfig) -> Self {
        Self { config }
    }

    /// Check if a path is a git repository
    pub fn is_git_repo(path: &Path) -> bool {
        path.join(".git").exists()
    }

    /// Find git repositories in a directory
    pub fn find_repos(&self, root: &Path, max_depth: usize) -> Vec<PathBuf> {
        let mut repos = Vec::new();
        self.find_repos_recursive(root, 0, max_depth, &mut repos);
        repos
    }

    fn find_repos_recursive(
        &self,
        path: &Path,
        depth: usize,
        max_depth: usize,
        repos: &mut Vec<PathBuf>,
    ) {
        if depth > max_depth || !path.is_dir() {
            return;
        }

        if Self::is_git_repo(path) {
            repos.push(path.to_path_buf());
            return; // Don't recurse into git repos
        }

        // Skip common non-project directories
        let skip_dirs = ["node_modules", "target", ".git", "vendor", ".venv", "venv"];
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if skip_dirs.contains(&name) || name.starts_with('.') {
                return;
            }
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    self.find_repos_recursive(&entry_path, depth + 1, max_depth, repos);
                }
            }
        }
    }

    /// Analyze a git repository
    pub fn analyze_repo(&self, repo_path: &Path) -> Result<GitSignal, String> {
        if !Self::is_git_repo(repo_path) {
            return Err(format!("Not a git repository: {}", repo_path.display()));
        }

        let repo_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Get recent commits
        let recent_commits = self.get_recent_commits(repo_path)?;

        // Get active branches
        let active_branches = self.get_branches(repo_path)?;

        // Calculate file activity
        let file_activity = self.calculate_file_activity(&recent_commits);

        // Extract topics from commits
        let extracted_topics = self.extract_topics_from_commits(&recent_commits);

        // Calculate commit frequency (commits per day)
        let commit_frequency = if recent_commits.is_empty() {
            0.0
        } else {
            recent_commits.len() as f32 / self.config.history_days as f32
        };

        // Get last commit hash
        let last_commit = recent_commits.first().map(|c| c.hash.clone());

        // Calculate confidence based on activity
        let confidence = self.calculate_confidence(&recent_commits, &active_branches);

        Ok(GitSignal {
            repo_path: repo_path.to_path_buf(),
            repo_name,
            recent_commits,
            active_branches,
            file_activity,
            extracted_topics,
            commit_frequency,
            last_commit,
            confidence,
        })
    }

    /// Get recent commits
    fn get_recent_commits(&self, repo_path: &Path) -> Result<Vec<CommitInfo>, String> {
        let since_date = format!("--since={} days ago", self.config.history_days);
        let max_count = format!("-{}", self.config.max_commits);

        let mut args = vec![
            "log",
            &max_count,
            &since_date,
            "--pretty=format:%H|%h|%s|%an|%ai",
            "--shortstat",
        ];

        if !self.config.include_merges {
            args.push("--no-merges");
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git log: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();
        let mut current_commit: Option<CommitInfo> = None;

        for line in stdout.lines() {
            if line.contains('|') && line.len() > 40 {
                // This is a commit line
                if let Some(commit) = current_commit.take() {
                    commits.push(commit);
                }

                let parts: Vec<&str> = line.splitn(5, '|').collect();
                if parts.len() >= 5 {
                    let message = parts[2].to_string();
                    let topics = extract_topics_from_commit_message(&message);

                    current_commit = Some(CommitInfo {
                        hash: parts[0].to_string(),
                        short_hash: parts[1].to_string(),
                        message,
                        author: parts[3].to_string(),
                        timestamp: parts[4].to_string(),
                        files_changed: Vec::new(),
                        insertions: 0,
                        deletions: 0,
                        topics,
                    });
                }
            } else if line.contains("file") && line.contains("changed") {
                // This is a stat line
                if let Some(ref mut commit) = current_commit {
                    parse_stat_line(line, commit);
                }
            }
        }

        // Don't forget the last commit
        if let Some(commit) = current_commit {
            commits.push(commit);
        }

        Ok(commits)
    }

    /// Get branch information
    fn get_branches(&self, repo_path: &Path) -> Result<Vec<BranchInfo>, String> {
        let output = Command::new("git")
            .args(["branch", "-v", "--no-color"])
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("Failed to run git branch: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "git branch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut branches = Vec::new();

        for line in stdout.lines() {
            let is_current = line.starts_with('*');
            let line = line.trim_start_matches(&['*', ' '][..]);

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                branches.push(BranchInfo {
                    name: parts[0].to_string(),
                    is_current,
                    last_commit: parts[1].to_string(),
                    ahead: 0,  // Would need additional git commands
                    behind: 0, // Would need additional git commands
                });
            }
        }

        Ok(branches)
    }

    /// Calculate file activity from commits
    fn calculate_file_activity(&self, commits: &[CommitInfo]) -> HashMap<String, u32> {
        let mut activity: HashMap<String, u32> = HashMap::new();

        for commit in commits {
            for file in &commit.files_changed {
                *activity.entry(file.clone()).or_insert(0) += 1;
            }
        }

        activity
    }

    /// Extract topics from all commits
    fn extract_topics_from_commits(&self, commits: &[CommitInfo]) -> Vec<String> {
        let mut topics: HashSet<String> = HashSet::new();

        for commit in commits {
            topics.extend(commit.topics.iter().cloned());
        }

        let mut topic_vec: Vec<_> = topics.into_iter().collect();
        topic_vec.sort();
        topic_vec
    }

    /// Calculate confidence based on repository activity
    fn calculate_confidence(&self, commits: &[CommitInfo], branches: &[BranchInfo]) -> f32 {
        if commits.is_empty() {
            return 0.1;
        }

        // Factors:
        // - Number of commits (more = higher confidence)
        // - Recency (more recent = higher confidence)
        // - Active branches (more = higher confidence)

        let commit_factor = (commits.len() as f32 / 20.0).min(1.0) * 0.4;
        let branch_factor = (branches.len() as f32 / 5.0).min(1.0) * 0.2;

        // Recency factor based on first (most recent) commit
        let recency_factor = 0.4; // Assume recent since we filtered by date

        (commit_factor + branch_factor + recency_factor).min(0.95)
    }
}

impl Default for GitAnalyzer {
    fn default() -> Self {
        Self::new(GitConfig::default())
    }
}

/// Extract topics from a commit message
fn extract_topics_from_commit_message(message: &str) -> Vec<String> {
    let mut topics = HashSet::new();
    let message_lower = message.to_lowercase();

    // Conventional commit prefixes
    let prefixes = [
        "feat", "fix", "docs", "style", "refactor", "test", "chore", "perf", "ci",
    ];
    for prefix in prefixes {
        if message_lower.starts_with(prefix) {
            topics.insert(format!("commit-{}", prefix));
        }
    }

    // Technology keywords
    let tech_keywords = [
        "api",
        "database",
        "db",
        "auth",
        "authentication",
        "ui",
        "frontend",
        "backend",
        "test",
        "testing",
        "docker",
        "kubernetes",
        "k8s",
        "aws",
        "deploy",
        "ci",
        "cd",
        "security",
        "performance",
        "cache",
        "async",
        "sync",
        "migration",
        "schema",
        "graphql",
        "rest",
        "grpc",
        "websocket",
        "webhook",
    ];

    for keyword in tech_keywords {
        if message_lower.contains(keyword) {
            topics.insert(keyword.to_string());
        }
    }

    topics.into_iter().collect()
}

/// Parse git stat line (e.g., " 3 files changed, 10 insertions(+), 5 deletions(-)")
fn parse_stat_line(line: &str, commit: &mut CommitInfo) {
    let parts: Vec<&str> = line.split(',').collect();

    for part in parts {
        let part = part.trim();
        if part.contains("insertion") {
            if let Some(num) = part.split_whitespace().next() {
                commit.insertions = num.parse().unwrap_or(0);
            }
        } else if part.contains("deletion") {
            if let Some(num) = part.split_whitespace().next() {
                commit.deletions = num.parse().unwrap_or(0);
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_topics_from_commit_message() {
        let topics = extract_topics_from_commit_message("feat: add authentication API");
        assert!(topics.contains(&"commit-feat".to_string()));
        assert!(topics.contains(&"auth".to_string()));
        assert!(topics.contains(&"api".to_string()));

        let topics = extract_topics_from_commit_message("fix: resolve database migration issue");
        assert!(topics.contains(&"commit-fix".to_string()));
        assert!(topics.contains(&"database".to_string()));
        assert!(topics.contains(&"migration".to_string()));
    }

    #[test]
    fn test_parse_stat_line() {
        let mut commit = CommitInfo {
            hash: String::new(),
            short_hash: String::new(),
            message: String::new(),
            author: String::new(),
            timestamp: String::new(),
            files_changed: Vec::new(),
            insertions: 0,
            deletions: 0,
            topics: Vec::new(),
        };

        parse_stat_line(
            " 3 files changed, 45 insertions(+), 12 deletions(-)",
            &mut commit,
        );
        assert_eq!(commit.insertions, 45);
        assert_eq!(commit.deletions, 12);
    }

    #[test]
    fn test_git_config_defaults() {
        let config = GitConfig::default();
        assert_eq!(config.max_commits, 100);
        assert_eq!(config.history_days, 30);
        assert!(!config.include_merges);
    }
}
