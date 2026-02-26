import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import type { CommitSummary } from '../../types/command-deck';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';

export function HistoryTab() {
  const { t } = useTranslation();
  const { commandHistory, loadCommandHistory, setCommandInput, setCommandDeckTab, selectedRepoPath } =
    useAppStore(
      useShallow((s) => ({
        commandHistory: s.commandHistory,
        loadCommandHistory: s.loadCommandHistory,
        setCommandInput: s.setCommandInput,
        setCommandDeckTab: s.setCommandDeckTab,
        selectedRepoPath: s.selectedRepoPath,
      })),
    );

  const [gitLog, setGitLog] = useState<CommitSummary[]>([]);

  useEffect(() => {
    loadCommandHistory();
  }, [loadCommandHistory]);

  useEffect(() => {
    if (!selectedRepoPath) return;
    invoke<CommitSummary[]>('git_deck_log', { repoPath: selectedRepoPath, count: 15 })
      .then(setGitLog)
      .catch(() => setGitLog([]));
  }, [selectedRepoPath]);

  return (
    <div className="flex flex-col gap-4 overflow-y-auto max-h-[calc(50vh-120px)]">
      {/* Command History */}
      <div>
        <h4 className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-2">
          {t('commandDeck.commands')}
        </h4>
        {commandHistory.length === 0 ? (
          <p className="text-xs text-gray-600">{t('commandDeck.history.noCommands')}</p>
        ) : (
          <div className="space-y-0.5">
            {commandHistory.slice(0, 20).map((entry) => (
              <button
                key={entry.id}
                onClick={() => {
                  setCommandInput(entry.command);
                  setCommandDeckTab('commands');
                }}
                className="flex items-center gap-2 w-full px-2 py-1 text-xs hover:bg-bg-tertiary rounded group"
              >
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    entry.success ? 'bg-green-500' : 'bg-red-500'
                  }`}
                />
                <span className="truncate flex-1 text-left font-mono text-gray-300">
                  {entry.command}
                </span>
                <span className="text-[10px] text-gray-600 group-hover:text-gray-400">
                  {new Date(entry.created_at).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Git Log */}
      {gitLog.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-gray-400 uppercase tracking-wider mb-2">
            {t('commandDeck.history.recentCommits')}
          </h4>
          <div className="space-y-0.5">
            {gitLog.map((commit) => (
              <div
                key={commit.hash}
                className="flex items-center gap-2 px-2 py-1 text-xs rounded"
              >
                <span className="text-[#D4AF37] font-mono text-[10px] flex-shrink-0">
                  {commit.short_hash}
                </span>
                <span className="truncate flex-1 text-gray-300">{commit.message}</span>
                <span className="text-[10px] text-gray-600 flex-shrink-0">
                  {commit.files_changed > 0 && `${commit.files_changed}f`}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
