import { useEffect, useCallback } from 'react';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { RepoSelector } from './RepoSelector';
import { GitStatusTab } from './GitStatusTab';
import { CommandRunnerTab } from './CommandRunnerTab';
import { HistoryTab } from './HistoryTab';
import type { CommandDeckTab } from '../../types/command-deck';

const TABS: { id: CommandDeckTab; label: string; key: string }[] = [
  { id: 'git', label: 'Git', key: '1' },
  { id: 'commands', label: 'Commands', key: '2' },
  { id: 'history', label: 'History', key: '3' },
];

export function CommandDeck() {
  const {
    commandDeckOpen,
    commandDeckTab,
    gitStatus,
    toggleCommandDeck,
    setCommandDeckTab,
  } = useAppStore(
    useShallow((s) => ({
      commandDeckOpen: s.commandDeckOpen,
      commandDeckTab: s.commandDeckTab,
      gitStatus: s.gitStatus,
      toggleCommandDeck: s.toggleCommandDeck,
      setCommandDeckTab: s.setCommandDeckTab,
    })),
  );

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
        const tab = TABS.find((t) => t.key === e.key);
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

  return (
    <div
      className="fixed bottom-0 left-0 right-0 z-40 bg-bg-secondary border-t border-border shadow-2xl transition-transform duration-200"
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
        <div className="flex items-center gap-0.5 bg-bg-tertiary rounded p-0.5">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setCommandDeckTab(tab.id)}
              className={`px-3 py-1 text-xs rounded transition-all ${
                commandDeckTab === tab.id
                  ? 'bg-bg-secondary text-white font-medium'
                  : 'text-gray-500 hover:text-gray-300'
              }`}
            >
              {tab.label}
              <span className="ml-1 text-[10px] text-gray-600">{tab.key}</span>
            </button>
          ))}
        </div>

        {/* Close */}
        <button
          onClick={toggleCommandDeck}
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
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">Ctrl+`</kbd> Toggle
        </span>
        <span>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">Esc</kbd> Close
        </span>
        <span>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded">1</kbd>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded ml-0.5">2</kbd>
          <kbd className="px-1 py-0.5 bg-bg-tertiary rounded ml-0.5">3</kbd> Tabs
        </span>
      </div>
    </div>
  );
}
