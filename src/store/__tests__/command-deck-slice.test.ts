import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('command-deck-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has commandDeckOpen false', () => {
      expect(useAppStore.getState().commandDeckOpen).toBe(false);
    });

    it('has commandDeckTab set to git', () => {
      expect(useAppStore.getState().commandDeckTab).toBe('git');
    });

    it('has empty repos', () => {
      expect(useAppStore.getState().repos).toEqual([]);
    });

    it('has selectedRepoPath null', () => {
      expect(useAppStore.getState().selectedRepoPath).toBeNull();
    });

    it('has gitStatus null', () => {
      expect(useAppStore.getState().gitStatus).toBeNull();
    });

    it('has gitStatusLoading false', () => {
      expect(useAppStore.getState().gitStatusLoading).toBe(false);
    });

    it('has empty commitMessage', () => {
      expect(useAppStore.getState().commitMessage).toBe('');
    });

    it('has suggestedCommitMessage null', () => {
      expect(useAppStore.getState().suggestedCommitMessage).toBeNull();
    });

    it('has suggestingCommit false', () => {
      expect(useAppStore.getState().suggestingCommit).toBe(false);
    });

    it('has empty commandInput', () => {
      expect(useAppStore.getState().commandInput).toBe('');
    });

    it('has commandOutput null', () => {
      expect(useAppStore.getState().commandOutput).toBeNull();
    });

    it('has commandRunning false', () => {
      expect(useAppStore.getState().commandRunning).toBe(false);
    });

    it('has empty commandHistory', () => {
      expect(useAppStore.getState().commandHistory).toEqual([]);
    });

    it('has confirmAction null', () => {
      expect(useAppStore.getState().confirmAction).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // toggleCommandDeck
  // ---------------------------------------------------------------------------
  describe('toggleCommandDeck', () => {
    it('toggles commandDeckOpen from false to true', () => {
      // loadRepos will be called when opening with no repos
      vi.mocked(invoke).mockResolvedValueOnce([]);

      useAppStore.getState().toggleCommandDeck();

      expect(useAppStore.getState().commandDeckOpen).toBe(true);
    });

    it('toggles commandDeckOpen from true to false', () => {
      useAppStore.setState({ commandDeckOpen: true });

      useAppStore.getState().toggleCommandDeck();

      expect(useAppStore.getState().commandDeckOpen).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // setCommandDeckTab
  // ---------------------------------------------------------------------------
  describe('setCommandDeckTab', () => {
    it('sets the active tab', () => {
      useAppStore.getState().setCommandDeckTab('commands');

      expect(useAppStore.getState().commandDeckTab).toBe('commands');
    });
  });

  // ---------------------------------------------------------------------------
  // setCommitMessage / setCommandInput / setConfirmAction
  // ---------------------------------------------------------------------------
  describe('simple setters', () => {
    it('setCommitMessage updates commitMessage', () => {
      useAppStore.getState().setCommitMessage('fix: typo');

      expect(useAppStore.getState().commitMessage).toBe('fix: typo');
    });

    it('setCommandInput updates commandInput', () => {
      useAppStore.getState().setCommandInput('ls -la');

      expect(useAppStore.getState().commandInput).toBe('ls -la');
    });

    it('setConfirmAction sets confirm action', () => {
      useAppStore.getState().setConfirmAction({ type: 'commit' });

      expect(useAppStore.getState().confirmAction).toEqual({ type: 'commit' });
    });

    it('setConfirmAction clears with null', () => {
      useAppStore.getState().setConfirmAction({ type: 'push' });
      useAppStore.getState().setConfirmAction(null);

      expect(useAppStore.getState().confirmAction).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadRepos
  // ---------------------------------------------------------------------------
  describe('loadRepos', () => {
    it('loads repos and auto-selects the first', async () => {
      const mockRepos = [
        { path: '/home/user/project', name: 'project' },
        { path: '/home/user/other', name: 'other' },
      ];
      // First call: git_deck_list_repos; second call: git_deck_status (auto-load)
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockRepos)
        .mockResolvedValueOnce({ staged: [], unstaged: [], branch: 'main' });

      await useAppStore.getState().loadRepos();

      expect(invoke).toHaveBeenCalledWith('git_deck_list_repos');
      expect(useAppStore.getState().repos).toEqual(mockRepos);
      expect(useAppStore.getState().selectedRepoPath).toBe('/home/user/project');
    });

    it('handles errors gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadRepos();

      expect(useAppStore.getState().repos).toEqual([]);
    });
  });

  // ---------------------------------------------------------------------------
  // loadGitStatus
  // ---------------------------------------------------------------------------
  describe('loadGitStatus', () => {
    it('loads git status for selected repo', async () => {
      useAppStore.setState({ selectedRepoPath: '/repo' });
      const mockStatus = { staged: ['file.ts'], unstaged: [], branch: 'main' };
      vi.mocked(invoke).mockResolvedValueOnce(mockStatus);

      await useAppStore.getState().loadGitStatus();

      expect(invoke).toHaveBeenCalledWith('git_deck_status', { repoPath: '/repo' });
      expect(useAppStore.getState().gitStatus).toEqual(mockStatus);
      expect(useAppStore.getState().gitStatusLoading).toBe(false);
    });

    it('does nothing without selected repo', async () => {
      useAppStore.setState({ selectedRepoPath: null });

      await useAppStore.getState().loadGitStatus();

      expect(invoke).not.toHaveBeenCalled();
    });

    it('resets loading on error', async () => {
      useAppStore.setState({ selectedRepoPath: '/repo' });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadGitStatus();

      expect(useAppStore.getState().gitStatusLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // runCommand
  // ---------------------------------------------------------------------------
  describe('runCommand', () => {
    it('runs command and sets output', async () => {
      useAppStore.setState({ commandInput: 'echo hello', selectedRepoPath: '/repo' });
      const mockOutput = { stdout: 'hello\n', stderr: '', exit_code: 0, duration_ms: 50 };
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockOutput) // run_shell_command
        .mockResolvedValueOnce([]);        // get_command_history

      await useAppStore.getState().runCommand();

      expect(invoke).toHaveBeenCalledWith('run_shell_command', {
        command: 'echo hello',
        workingDir: '/repo',
      });
      expect(useAppStore.getState().commandOutput).toEqual(mockOutput);
      expect(useAppStore.getState().commandRunning).toBe(false);
    });

    it('does nothing when commandInput is empty', async () => {
      useAppStore.setState({ commandInput: '   ' });

      await useAppStore.getState().runCommand();

      expect(invoke).not.toHaveBeenCalled();
    });

    it('sets error output on failure', async () => {
      useAppStore.setState({ commandInput: 'bad-cmd', selectedRepoPath: '/repo' });
      vi.mocked(invoke).mockRejectedValueOnce('Command not found');

      await useAppStore.getState().runCommand();

      const output = useAppStore.getState().commandOutput;
      expect(output).not.toBeNull();
      expect(output!.exit_code).toBe(-1);
      expect(output!.stderr).toContain('Command not found');
      expect(useAppStore.getState().commandRunning).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // suggestCommitMessage
  // ---------------------------------------------------------------------------
  describe('suggestCommitMessage', () => {
    it('sets suggested message and commit message', async () => {
      useAppStore.setState({ selectedRepoPath: '/repo' });
      const mockSuggestion = { message: 'feat: add new feature', reasoning: 'Based on staged changes' };
      vi.mocked(invoke).mockResolvedValueOnce(mockSuggestion);

      await useAppStore.getState().suggestCommitMessage();

      expect(invoke).toHaveBeenCalledWith('git_deck_suggest_commit', { repoPath: '/repo' });
      expect(useAppStore.getState().suggestedCommitMessage).toEqual(mockSuggestion);
      expect(useAppStore.getState().commitMessage).toBe('feat: add new feature');
      expect(useAppStore.getState().suggestingCommit).toBe(false);
    });

    it('does nothing without selected repo', async () => {
      useAppStore.setState({ selectedRepoPath: null });

      await useAppStore.getState().suggestCommitMessage();

      expect(invoke).not.toHaveBeenCalled();
    });

    it('resets suggestingCommit on error', async () => {
      useAppStore.setState({ selectedRepoPath: '/repo' });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().suggestCommitMessage();

      expect(useAppStore.getState().suggestingCommit).toBe(false);
    });
  });
});
