// Command Deck types — mirrors Rust structs from git_deck.rs and command_runner.rs

export type CommandDeckTab = 'git' | 'commands' | 'history';

export interface CommandOutput {
  stdout: string;
  stderr: string;
  exit_code: number;
  duration_ms: number;
}

export interface GitDeckStatus {
  branch: string;
  ahead: number;
  behind: number;
  staged: FileChange[];
  unstaged: FileChange[];
  untracked: string[];
  has_conflicts: boolean;
}

export interface FileChange {
  path: string;
  status: string; // "M", "A", "D", "R", "C", "U"
}

export interface CommitResult {
  hash: string;
  short_hash: string;
  message: string;
  files_changed: number;
}

export interface CommitSummary {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  date: string;
  files_changed: number;
}

export interface SuggestedCommitMessage {
  message: string;
  summary: string;
  model: string | null;
}

export interface CommandHistoryEntry {
  id: number;
  command: string;
  working_dir: string;
  exit_code: number | null;
  success: boolean;
  output_preview: string | null;
  created_at: string;
}

export interface RepoInfo {
  path: string;
  name: string;
  branch: string | null;
  has_changes: boolean;
}
