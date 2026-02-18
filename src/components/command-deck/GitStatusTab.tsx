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
  const {
    gitStatus,
    gitStatusLoading,
    commitMessage,
    setCommitMessage,
    suggestedCommitMessage,
    suggestingCommit,
    confirmAction,
    setConfirmAction,
    stageFiles,
    unstageFiles,
    commitChanges,
    pushChanges,
    suggestCommitMessage,
    loadGitStatus,
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
      stageFiles: s.stageFiles,
      unstageFiles: s.unstageFiles,
      commitChanges: s.commitChanges,
      pushChanges: s.pushChanges,
      suggestCommitMessage: s.suggestCommitMessage,
      loadGitStatus: s.loadGitStatus,
    })),
  );

  if (gitStatusLoading) {
    return (
      <div className="flex items-center justify-center py-8 text-gray-500 text-sm">
        Loading git status...
      </div>
    );
  }

  if (!gitStatus) {
    return (
      <div className="flex items-center justify-center py-8 text-gray-500 text-sm">
        Select a repository to view status
      </div>
    );
  }

  const hasStaged = gitStatus.staged.length > 0;
  const hasUnstaged = gitStatus.unstaged.length > 0;
  const hasUntracked = gitStatus.untracked.length > 0;
  const isClean = !hasStaged && !hasUnstaged && !hasUntracked;

  return (
    <div className="flex flex-col gap-3 overflow-y-auto max-h-[calc(50vh-120px)]">
      {/* Conflicts warning */}
      {gitStatus.has_conflicts && (
        <div className="px-3 py-2 bg-red-500/10 border border-red-500/30 rounded-lg text-xs text-red-400">
          Merge conflicts detected. Resolve before committing.
        </div>
      )}

      {isClean && (
        <div className="text-center py-4 text-gray-500 text-sm">
          Working tree clean
        </div>
      )}

      {/* Staged files */}
      {hasStaged && (
        <div>
          <div className="flex items-center justify-between mb-1">
            <h4 className="text-xs font-medium text-green-400 uppercase tracking-wider">
              Staged ({gitStatus.staged.length})
            </h4>
          </div>
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
          <div className="flex items-center justify-between mb-1">
            <h4 className="text-xs font-medium text-orange-400 uppercase tracking-wider">
              Modified ({gitStatus.unstaged.length})
            </h4>
            <button
              onClick={() => stageFiles(gitStatus.unstaged.map((f) => f.path))}
              className="text-[10px] text-gray-500 hover:text-white transition-colors"
            >
              Stage All
            </button>
          </div>
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
          <div className="flex items-center justify-between mb-1">
            <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wider">
              Untracked ({gitStatus.untracked.length})
            </h4>
            <button
              onClick={() => stageFiles(gitStatus.untracked)}
              className="text-[10px] text-gray-500 hover:text-white transition-colors"
            >
              Stage All
            </button>
          </div>
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
          <div className="flex items-center gap-2">
            <textarea
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && e.ctrlKey && commitMessage.trim()) {
                  setConfirmAction({ type: 'commit' });
                }
              }}
              placeholder="Commit message..."
              className="flex-1 bg-bg-tertiary border border-border rounded px-3 py-2 text-sm text-white placeholder-gray-600 resize-none h-16 focus:outline-none focus:border-gray-500 font-mono"
            />
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={suggestCommitMessage}
              disabled={suggestingCommit}
              className="px-3 py-1.5 text-xs text-gray-400 border border-border rounded hover:border-gray-500 hover:text-white transition-all disabled:opacity-50"
            >
              {suggestingCommit ? 'Thinking...' : 'AI Suggest'}
            </button>
            {suggestedCommitMessage?.model && (
              <span className="text-[10px] text-gray-600">via {suggestedCommitMessage.model}</span>
            )}
            <div className="flex-1" />
            <button
              onClick={() => setConfirmAction({ type: 'commit' })}
              disabled={!commitMessage.trim()}
              className="px-4 py-1.5 text-xs font-medium text-black bg-white rounded hover:bg-gray-200 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            >
              Commit
            </button>
          </div>

          {confirmAction?.type === 'commit' && (
            <ConfirmGate
              message={`Commit ${gitStatus.staged.length} file(s)?`}
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
              message={`Push ${gitStatus.ahead} commit(s) to origin?`}
              onConfirm={pushChanges}
              onCancel={() => setConfirmAction(null)}
            />
          ) : (
            <button
              onClick={() => setConfirmAction({ type: 'push' })}
              className="flex items-center gap-2 px-4 py-2 text-sm bg-bg-tertiary border border-border rounded-lg hover:border-[#D4AF37]/40 transition-all w-full"
            >
              <span className="text-white">Push</span>
              <span className="text-xs text-[#D4AF37]">{gitStatus.ahead} ahead</span>
            </button>
          )}
        </div>
      )}

      {/* Refresh */}
      <button
        onClick={loadGitStatus}
        className="text-[10px] text-gray-600 hover:text-gray-400 transition-colors self-center"
      >
        Refresh
      </button>
    </div>
  );
}
