import { useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';

export function CommandRunnerTab() {
  const { t } = useTranslation();
  const {
    commandInput,
    setCommandInput,
    commandOutput,
    commandRunning,
    commandHistory,
    selectedRepoPath,
    runCommand,
    loadCommandHistory,
  } = useAppStore(
    useShallow((s) => ({
      commandInput: s.commandInput,
      setCommandInput: s.setCommandInput,
      commandOutput: s.commandOutput,
      commandRunning: s.commandRunning,
      commandHistory: s.commandHistory,
      selectedRepoPath: s.selectedRepoPath,
      runCommand: s.runCommand,
      loadCommandHistory: s.loadCommandHistory,
    })),
  );

  const inputRef = useRef<HTMLInputElement>(null);
  const [historyIndex, setHistoryIndex] = useState(-1);

  // Load history on mount
  const loadedRef = useRef(false);
  if (!loadedRef.current) {
    loadedRef.current = true;
    loadCommandHistory();
  }

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !commandRunning && commandInput.trim()) {
        e.preventDefault();
        setHistoryIndex(-1);
        runCommand();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (commandHistory.length > 0) {
          const next = Math.min(historyIndex + 1, commandHistory.length - 1);
          setHistoryIndex(next);
          setCommandInput(commandHistory[next].command);
        }
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (historyIndex > 0) {
          const next = historyIndex - 1;
          setHistoryIndex(next);
          setCommandInput(commandHistory[next].command);
        } else {
          setHistoryIndex(-1);
          setCommandInput('');
        }
      }
    },
    [commandRunning, commandInput, commandHistory, historyIndex, runCommand, setCommandInput],
  );

  return (
    <div className="flex flex-col gap-3 h-full">
      {/* Working directory */}
      <div className="text-[10px] text-gray-600 font-mono truncate">
        {selectedRepoPath || t('commandRunner.noDir')}
      </div>

      {/* Command input */}
      <div className="flex items-center gap-2">
        <span className="text-green-500 text-sm font-mono">$</span>
        <input
          ref={inputRef}
          type="text"
          value={commandInput}
          onChange={(e) => setCommandInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('commandRunner.placeholder')}
          disabled={commandRunning}
          className="flex-1 bg-bg-tertiary border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-600 font-mono focus:outline-none focus:border-gray-500 disabled:opacity-50"
          autoFocus
        />
        {commandRunning && (
          <div className="w-4 h-4 border-2 border-gray-500 border-t-white rounded-full animate-spin" />
        )}
      </div>

      {/* Output area */}
      {commandOutput && (
        <div className="flex-1 overflow-y-auto bg-bg-primary border border-border rounded-lg p-3 min-h-[100px] max-h-[calc(50vh-200px)]">
          {commandOutput.stdout && (
            <pre className="text-xs text-gray-300 font-mono whitespace-pre-wrap break-all">
              {commandOutput.stdout}
            </pre>
          )}
          {commandOutput.stderr && (
            <pre className="text-xs text-red-400 font-mono whitespace-pre-wrap break-all mt-1">
              {commandOutput.stderr}
            </pre>
          )}
          <div className="flex items-center gap-3 mt-2 pt-2 border-t border-border">
            <span
              className={`text-[10px] font-mono ${
                commandOutput.exit_code === 0 ? 'text-green-500' : 'text-red-400'
              }`}
            >
              {t('commandRunner.exit', { code: commandOutput.exit_code })}
            </span>
            <span className="text-[10px] text-gray-600">{commandOutput.duration_ms}ms</span>
          </div>
        </div>
      )}

      {!commandOutput && !commandRunning && (
        <div className="flex-1 flex items-center justify-center text-gray-600 text-xs">
          {t('commandRunner.hint')}
        </div>
      )}
    </div>
  );
}
