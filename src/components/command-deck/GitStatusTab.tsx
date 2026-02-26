import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { ConfirmGate } from './ConfirmGate';

function StatusDot({ status }: { status: string }) {
  const color =
    status === 'A' ? 'bg-green-500' :
    status === 'D' ? 'bg-red-500' :
    status === 'U' ? 'bg-yellow-500' :
    'bg-orange-400';
  return <span className={`inline-block w-2 h-2 rounded-full ${color}`} />;
}

export function GitStatusTab() {
  const { t } = useTranslation();
  const {
    gitStatus,
    gitStatusLoading,
    commitMessage,
    setCommitMessage,
    suggestedCommitMessage,
    suggestingCommit,
    confirmAction,
    setConfirmAction,
    selectedRepoPath,
    stageFiles,
    unstageFiles,
    commitChanges,
    pushChanges,
    suggestCommitMessage,
  } = useAppStore(
    useShallow((s) => ({
      gitStatus: s.gitStatus,
      gitStatusLoading: s.gitStatusLoading,
      commitMessage: s.commitMessage,
      setCommitMessage: s.setCommitMessage,
      suggestedCommitMessage: s.suggestedCommitMessage,
      suggestingCommit: s.suggestingCommit,
      confirmAction: s.confirmAction,
      setConfirmAction: s.setConfirmAction,
      selectedRepoPath: s.selectedRepoPath,
      stageFiles: s.stageFiles,
      unstageFiles: s.unstageFiles,
      commitChanges: s.commitChanges,
      pushChanges: s.pushChanges,
      suggestCommitMessage: s.suggestCommitMessage,
    })),
  );

  // Diff stat preview for staged changes
  const [diffStat, setDiffStat] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);

  useEffect(() => {
    if (!selectedRepoPath || !gitStatus?.staged.length) {
      setDiffStat(null);
      return;
    }
    invoke<string>('git_deck_diff_stat', { repoPath: selectedRepoPath, staged: true })
      .then((stat) => setDiffStat(stat.trim() || null))
      .catch(() => setDiffStat(null));
  }, [selectedRepoPath, gitStatus?.staged.length]);

  if (gitStatusLoading && !gitStatus) {
    return (
      <div className="flex items-center justify-center py-8 text-gray-500 text-sm">
        <div className="w-4 h-4 border-2 border-gray-600 border-t-gray-300 rounded-full animate-spin mr-2" />
        {t('commandDeck.git.loading')}
      </div>
    );
  }

  if (!gitStatus) {
    return (
      <div className="flex flex-col items-center justify-center py-8 gap-2">
        <span className="text-gray-500 text-sm">{t('commandDeck.git.selectRepo')}</span>
        <span className="text-gray-600 text-xs">{t('commandDeck.git.repoHint')}</span>
      </div>
    );
  }

  const hasStaged = gitStatus.staged.length > 0;
  const hasUnstaged = gitStatus.unstaged.length > 0;
  const hasUntracked = gitStatus.untracked.length > 0;
  const isClean = !hasStaged && !hasUnstaged && !hasUntracked;
  const allUnstagedPaths = [
    ...gitStatus.unstaged.map((f) => f.path),
    ...gitStatus.untracked,
  ];

  return (
    <div className="flex flex-col gap-3 overflow-y-auto max-h-[calc(50vh-120px)]">
      {/* Conflicts warning */}
      {gitStatus.has_conflicts && (
        <div className="px-3 py-2 bg-red-500/10 border border-red-500/30 rounded-lg text-xs text-red-400">
          {t('commandDeck.git.mergeConflicts')}
        </div>
      )}

      {isClean && (
        <div className="flex flex-col items-center py-6 gap-1">
          <span className="text-green-500 text-lg">&#10003;</span>
          <span className="text-gray-400 text-sm">{t('commandDeck.git.workingTreeClean')}</span>
          <span className="text-gray-600 text-xs">{t('commandDeck.git.nothingToCommit')}</span>
        </div>
      )}

      {/* Stage All bar (when there are unstaged changes) */}
      {allUnstagedPaths.length > 0 && (
        <div className="flex items-center justify-between px-2 py-1.5 bg-bg-tertiary/50 rounded">
          <span className="text-xs text-gray-400">
            {t('commandDeck.git.unstagedChanges', { count: allUnstagedPaths.length })}
          </span>
          <button
            onClick={() => stageFiles(allUnstagedPaths)}
            className="px-2 py-0.5 text-[10px] font-medium text-white bg-bg-tertiary border border-border rounded hover:border-gray-500 transition-colors"
          >
            {t('commandDeck.git.stageAll')}
          </button>
        </div>
      )}

      {/* Staged files */}
      {hasStaged && (
        <div>
          <h4 className="text-xs font-medium text-green-400 uppercase tracking-wider mb-1">
            {t('commandDeck.git.staged', { count: gitStatus.staged.length })}
          </h4>
          <div className="space-y-0.5">
            {gitStatus.staged.map((f) => (
              <button
                key={f.path}
                onClick={() => unstageFiles([f.path])}
                className="flex items-center gap-2 w-full px-2 py-1 text-xs text-gray-300 hover:bg-bg-tertiary rounded group"
                title="Click to unstage"
              >
                <StatusDot status={f.status} />
                <span className="truncate flex-1 text-left font-mono">{f.path}</span>
                <span className="text-gray-600 group-hover:text-gray-400 text-[10px]">unstage</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Unstaged files */}
      {hasUnstaged && (
        <div>
          <h4 className="text-xs font-medium text-orange-400 uppercase tracking-wider mb-1">
            {t('commandDeck.git.modified', { count: gitStatus.unstaged.length })}
          </h4>
          <div className="space-y-0.5">
            {gitStatus.unstaged.map((f) => (
              <button
                key={f.path}
                onClick={() => stageFiles([f.path])}
                className="flex items-center gap-2 w-full px-2 py-1 text-xs text-gray-300 hover:bg-bg-tertiary rounded group"
                title="Click to stage"
              >
                <StatusDot status={f.status} />
                <span className="truncate flex-1 text-left font-mono">{f.path}</span>
                <span className="text-gray-600 group-hover:text-gray-400 text-[10px]">stage</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Untracked files */}
      {hasUntracked && (
        <div>
          <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">
            {t('commandDeck.git.untracked', { count: gitStatus.untracked.length })}
          </h4>
          <div className="space-y-0.5">
            {gitStatus.untracked.map((path) => (
              <button
                key={path}
                onClick={() => stageFiles([path])}
                className="flex items-center gap-2 w-full px-2 py-1 text-xs text-gray-400 hover:bg-bg-tertiary rounded group"
                title="Click to stage"
              >
                <span className="inline-block w-2 h-2 rounded-full bg-gray-600" />
                <span className="truncate flex-1 text-left font-mono">{path}</span>
                <span className="text-gray-600 group-hover:text-gray-400 text-[10px]">stage</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Commit Area */}
      {hasStaged && (
        <div className="border-t border-border pt-3 space-y-2">
          {/* Diff stat preview */}
          {diffStat && (
            <div>
              <button
                onClick={() => setShowDiff(!showDiff)}
                className="flex items-center gap-1 text-[10px] text-gray-500 hover:text-gray-300 transition-colors mb-1"
              >
                <span className={`transition-transform ${showDiff ? 'rotate-90' : ''}`}>&#9654;</span>
                {t('commandDeck.git.diffSummary')}
              </button>
              {showDiff && (
                <pre className="text-[11px] text-gray-400 font-mono bg-bg-primary border border-border rounded p-2 overflow-x-auto max-h-24">
                  {diffStat}
                </pre>
              )}
            </div>
          )}

          <textarea
            value={commitMessage}
            onChange={(e) => setCommitMessage(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && e.ctrlKey && commitMessage.trim()) {
                setConfirmAction({ type: 'commit' });
              }
            }}
            placeholder={t('commandDeck.git.commitPlaceholder')}
            className="w-full bg-bg-tertiary border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-600 resize-none h-16 focus:outline-none focus:border-gray-500 font-mono"
          />
          <div className="flex items-center gap-2">
            <button
              onClick={suggestCommitMessage}
              disabled={suggestingCommit}
              className="px-3 py-1.5 text-xs text-gray-400 border border-border rounded hover:border-gray-500 hover:text-white transition-all disabled:opacity-50"
            >
              {suggestingCommit ? t('commandDeck.git.thinking') : t('commandDeck.git.aiSuggest')}
            </button>
            {suggestedCommitMessage?.model && (
              <span className="text-[10px] text-gray-600">via {suggestedCommitMessage.model}</span>
            )}
            <div className="flex-1" />
            <span className="text-[10px] text-gray-600">{t('commandDeck.git.ctrlEnterCommit')}</span>
            <button
              onClick={() => setConfirmAction({ type: 'commit' })}
              disabled={!commitMessage.trim()}
              className="px-4 py-1.5 text-xs font-medium text-black bg-white rounded hover:bg-gray-200 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            >
              {t('commandDeck.git.commit')}
            </button>
          </div>

          {confirmAction?.type === 'commit' && (
            <ConfirmGate
              message={t('commandDeck.git.confirmCommit', { count: gitStatus.staged.length })}
              onConfirm={commitChanges}
              onCancel={() => setConfirmAction(null)}
            />
          )}
        </div>
      )}

      {/* Push Area */}
      {gitStatus.ahead > 0 && (
        <div className="border-t border-border pt-3">
          {confirmAction?.type === 'push' ? (
            <ConfirmGate
              message={t('commandDeck.git.confirmPush', { count: gitStatus.ahead, branch: gitStatus.branch })}
              onConfirm={pushChanges}
              onCancel={() => setConfirmAction(null)}
            />
          ) : (
            <button
              onClick={() => setConfirmAction({ type: 'push' })}
              className="flex items-center gap-2 px-4 py-2 text-sm bg-bg-tertiary border border-border rounded-lg hover:border-[#D4AF37]/40 transition-all w-full"
            >
              <span className="text-white">{t('commandDeck.git.push')}</span>
              <span className="text-xs text-[#D4AF37]">{t('commandDeck.git.ahead', { count: gitStatus.ahead })}</span>
              <span className="text-[10px] text-gray-600 ml-auto">origin/{gitStatus.branch}</span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}
