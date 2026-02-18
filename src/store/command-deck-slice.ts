import { invoke } from '@tauri-apps/api/core';
import type { StateCreator } from 'zustand';
import type { AppStore } from './types';
import type {
  CommandDeckTab,
  CommandHistoryEntry,
  CommandOutput,
  GitDeckStatus,
  RepoInfo,
  SuggestedCommitMessage,
} from '../types/command-deck';

export interface CommandDeckSlice {
  // State
  commandDeckOpen: boolean;
  commandDeckTab: CommandDeckTab;
  repos: RepoInfo[];
  selectedRepoPath: string | null;
  gitStatus: GitDeckStatus | null;
  gitStatusLoading: boolean;
  commitMessage: string;
  suggestedCommitMessage: SuggestedCommitMessage | null;
  suggestingCommit: boolean;
  commandInput: string;
  commandOutput: CommandOutput | null;
  commandRunning: boolean;
  commandHistory: CommandHistoryEntry[];
  confirmAction: { type: 'commit' | 'push' } | null;

  // Actions
  toggleCommandDeck: () => void;
  setCommandDeckTab: (tab: CommandDeckTab) => void;
  setSelectedRepo: (path: string) => void;
  setCommitMessage: (msg: string) => void;
  setCommandInput: (cmd: string) => void;
  setConfirmAction: (action: { type: 'commit' | 'push' } | null) => void;

  // Async actions
  loadRepos: () => Promise<void>;
  loadGitStatus: () => Promise<void>;
  stageFiles: (paths: string[]) => Promise<void>;
  unstageFiles: (paths: string[]) => Promise<void>;
  commitChanges: () => Promise<void>;
  pushChanges: () => Promise<void>;
  suggestCommitMessage: () => Promise<void>;
  runCommand: () => Promise<void>;
  loadCommandHistory: () => Promise<void>;
}

export const createCommandDeckSlice: StateCreator<AppStore, [], [], CommandDeckSlice> = (set, get) => ({
  // Initial state
  commandDeckOpen: false,
  commandDeckTab: 'git',
  repos: [],
  selectedRepoPath: null,
  gitStatus: null,
  gitStatusLoading: false,
  commitMessage: '',
  suggestedCommitMessage: null,
  suggestingCommit: false,
  commandInput: '',
  commandOutput: null,
  commandRunning: false,
  commandHistory: [],
  confirmAction: null,

  toggleCommandDeck: () => {
    const open = !get().commandDeckOpen;
    set({ commandDeckOpen: open });
    if (open && get().repos.length === 0) {
      get().loadRepos();
    }
  },

  setCommandDeckTab: (tab) => set({ commandDeckTab: tab }),

  setSelectedRepo: (path) => {
    set({ selectedRepoPath: path, gitStatus: null });
    // Auto-load status for the selected repo
    setTimeout(() => get().loadGitStatus(), 0);
  },

  setCommitMessage: (msg) => set({ commitMessage: msg }),
  setCommandInput: (cmd) => set({ commandInput: cmd }),
  setConfirmAction: (action) => set({ confirmAction: action }),

  loadRepos: async () => {
    try {
      const repos = await invoke<RepoInfo[]>('git_deck_list_repos');
      set({ repos });
      // Auto-select first repo if none selected
      if (!get().selectedRepoPath && repos.length > 0) {
        set({ selectedRepoPath: repos[0].path });
        get().loadGitStatus();
      }
    } catch (e) {
      console.error('Failed to load repos:', e);
    }
  },

  loadGitStatus: async () => {
    const repoPath = get().selectedRepoPath;
    if (!repoPath) return;
    set({ gitStatusLoading: true });
    try {
      const status = await invoke<GitDeckStatus>('git_deck_status', { repoPath });
      set({ gitStatus: status, gitStatusLoading: false });
    } catch (e) {
      console.error('Failed to load git status:', e);
      set({ gitStatusLoading: false });
    }
  },

  stageFiles: async (paths) => {
    const repoPath = get().selectedRepoPath;
    if (!repoPath) return;
    try {
      await invoke('git_deck_stage', { repoPath, paths });
      get().loadGitStatus();
    } catch (e) {
      get().addToast('error', `Stage failed: ${e}`);
    }
  },

  unstageFiles: async (paths) => {
    const repoPath = get().selectedRepoPath;
    if (!repoPath) return;
    try {
      await invoke('git_deck_unstage', { repoPath, paths });
      get().loadGitStatus();
    } catch (e) {
      get().addToast('error', `Unstage failed: ${e}`);
    }
  },

  commitChanges: async () => {
    const { selectedRepoPath: repoPath, commitMessage: message } = get();
    if (!repoPath || !message.trim()) return;
    set({ confirmAction: null });
    try {
      const result = await invoke<{ short_hash: string; files_changed: number }>('git_deck_commit', {
        repoPath,
        message: message.trim(),
      });
      get().addToast('success', `Committed ${result.short_hash} (${result.files_changed} files)`);
      set({ commitMessage: '', suggestedCommitMessage: null });
      get().loadGitStatus();
    } catch (e) {
      get().addToast('error', `Commit failed: ${e}`);
    }
  },

  pushChanges: async () => {
    const repoPath = get().selectedRepoPath;
    if (!repoPath) return;
    set({ confirmAction: null });
    try {
      await invoke('git_deck_push', { repoPath, branch: null });
      get().addToast('success', 'Pushed successfully');
      get().loadGitStatus();
    } catch (e) {
      get().addToast('error', `Push failed: ${e}`);
    }
  },

  suggestCommitMessage: async () => {
    const repoPath = get().selectedRepoPath;
    if (!repoPath) return;
    set({ suggestingCommit: true });
    try {
      const suggestion = await invoke<SuggestedCommitMessage>('git_deck_suggest_commit', { repoPath });
      set({
        suggestedCommitMessage: suggestion,
        commitMessage: suggestion.message,
        suggestingCommit: false,
      });
    } catch (e) {
      set({ suggestingCommit: false });
      get().addToast('error', `Suggest failed: ${e}`);
    }
  },

  runCommand: async () => {
    const { commandInput, selectedRepoPath } = get();
    if (!commandInput.trim()) return;
    set({ commandRunning: true, commandOutput: null });
    try {
      const output = await invoke<CommandOutput>('run_shell_command', {
        command: commandInput.trim(),
        workingDir: selectedRepoPath,
      });
      set({ commandOutput: output, commandRunning: false });
      get().loadCommandHistory();
    } catch (e) {
      set({
        commandOutput: {
          stdout: '',
          stderr: String(e),
          exit_code: -1,
          duration_ms: 0,
        },
        commandRunning: false,
      });
    }
  },

  loadCommandHistory: async () => {
    try {
      const history = await invoke<CommandHistoryEntry[]>('get_command_history', { limit: 50 });
      set({ commandHistory: history });
    } catch (e) {
      console.error('Failed to load command history:', e);
    }
  },
});
