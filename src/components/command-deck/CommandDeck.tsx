import { useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { RepoSelector } from './RepoSelector';
import { GitStatusTab } from './GitStatusTab';
import { CommandRunnerTab } from './CommandRunnerTab';
import { HistoryTab } from './HistoryTab';
import type { CommandDeckTab } from '../../types/command-deck';

const TAB_KEYS: { id: CommandDeckTab; labelKey: string; key: string }[] = [
  { id: 'git', labelKey: 'commandDeck.git', key: '1' },
  { id: 'commands', labelKey: 'commandDeck.commands', key: '2' },
  { id: 'history', labelKey: 'commandDeck.history', key: '3' },
];

const GIT_REFRESH_INTERVAL = 5000; // Auto-refresh git status every 5s

export function CommandDeck() {
  const { t } = useTranslation();
  const {
    commandDeckOpen,
    commandDeckTab,
    gitStatus,
    selectedRepoPath,
    toggleCommandDeck,
    setCommandDeckTab,
    loadGitStatus,
  } = useAppStore(
    useShallow((s) => ({
      commandDeckOpen: s.commandDeckOpen,
      commandDeckTab: s.commandDeckTab,
      gitStatus: s.gitStatus,
      selectedRepoPath: s.selectedRepoPath,
      toggleCommandDeck: s.toggleCommandDeck,
      setCommandDeckTab: s.setCommandDeckTab,
      loadGitStatus: s.loadGitStatus,
    })),
  );

  // Auto-refresh git status when deck is open and git tab is active
  const intervalRef = useRef<ReturnType<typeof setInterval>>(undefined);
  useEffect(() => {
    if (commandDeckOpen && commandDeckTab === 'git' && selectedRepoPath) {
      intervalRef.current = setInterval(loadGitStatus, GIT_REFRESH_INTERVAL);
      return () => clearInterval(intervalRef.current);
    }
    clearInterval(intervalRef.current);
  }, [commandDeckOpen, commandDeckTab, selectedRepoPath, loadGitStatus]);

  // Keyboard shortcuts within the deck
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!commandDeckOpen) return;

      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        toggleCommandDeck();
        return;
      }

      // Tab switching: 1/2/3 only when not in an input
      const tag = (e.target as HTMLElement).tagName;
      if (tag !== 'INPUT' && tag !== 'TEXTAREA' && tag !== 'SELECT') {
        const tab = TAB_KEYS.find((t) => t.key === e.key);
        if (tab) {
          e.preventDefault();
          setCommandDeckTab(tab.id);
        }
      }
    },
    [commandDeckOpen, toggleCommandDeck, setCommandDeckTab],
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown, true); // capture phase
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [handleKeyDown]);

  if (!commandDeckOpen) return null;

  const totalChanges = gitStatus
    ? gitStatus.staged.length + gitStatus.unstaged.length + gitStatus.untracked.length
    : 0;

  return (
    <div
      className="fixed bottom-0 left-0 right-0 z-40 bg-bg-secondary border-t border-border shadow-2xl transition-transform duration-200"
      role="region"
      aria-label={t('commandDeck.title')}
      style={{ height: '50vh' }}
    >
      {/* Header */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-bg-primary/50">
        {/* Repo Selector */}
        <RepoSelector />

        {/* Branch + ahead/behind */}
        {gitStatus && (
          <div className="flex items-center gap-2">
            <span className="px-2 py-0.5 bg-bg-tertiary border border-border rounded text-xs font-mono text-white">
              {gitStatus.branch}
            </span>
            {(gitStatus.ahead > 0 || gitStatus.behind > 0) && (
              <span className="text-[10px] text-gray-500">
                {gitStatus.ahead > 0 && (
                  <span className="text-green-400">+{gitStatus.ahead}</span>
                )}
                {gitStatus.ahead > 0 && gitStatus.behind > 0 && ' '}
                {gitStatus.behind > 0 && (
                  <span className="text-red-400">-{gitStatus.behind}</span>
                )}
              </span>
            )}
          </div>
        )}

        {/* Spacer */}
        <div className="flex-1" />

        {/* Tab Bar */}
        <div className="flex items-center gap-0.5 bg-bg-tertiary rounded p-0.5" role="tablist" aria-label={t('commandDeck.title')}>
          {TAB_KEYS.map((tab) => (
            <button
              key={tab.id}
              role="tab"
              aria-selected={commandDeckTab === tab.id}
              onClick={() => setCommandDeckTab(tab.id)}
              className={`px-3 py-1 text-xs rounded transition-all ${
                commandDeckTab === tab.id
                  ? 'bg-bg-secondary text-white font-medium'
                  : 'text-gray-500 hover:text-gray-300'
              }`}
            >
              {t(tab.labelKey)}
              {tab.id === 'git' && totalChanges > 0 && (
                <span className="ml-1 px-1.5 py-0.5 bg-orange-500/20 text-orange-400 text-[10px] rounded-full">
                  {totalChanges}
                </span>
              )}
              <span className="ml-1 text-[10px] text-gray-600">{tab.key}</span>
            </button>
          ))}
        </div>

        {/* Close */}
        <button
          onClick={toggleCommandDeck}
          aria-label={t('action.close')}
          className="w-7 h-7 rounded bg-bg-tertiary text-gray-500 hover:text-white hover:bg-border flex items-center justify-center transition-all text-sm"
        >
          &times;
        </button>
      </div>

      {/* Content */}
      <div className="p-4 h-[calc(100%-80px)] overflow-hidden">
        {commandDeckTab === 'git' && <GitStatusTab />}
        {commandDeckTab === 'commands' && <CommandRunnerTab />}
        {commandDeckTab === 'history' && <HistoryTab />}
      </div>

      {/* Footer */}
      <div className="absolute bottom-0 left-0 right-0 px-4 py-1.5 bg-bg-primary/80 border-t border-border flex items-center gap-4 text-[10px] text-gray-600">
        <span>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">Ctrl+`</kbd> {t('commandDeck.toggle')}
        </span>
        <span>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">Esc</kbd> {t('action.close')}
        </span>
        <span>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">1</kbd>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded ml-0.5">2</kbd>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded ml-0.5">3</kbd> {t('commandDeck.tabs')}
        </span>
      </div>
    </div>
  );
}
