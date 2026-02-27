import { useState, useEffect, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import type { AgentMemoryEntry } from '../store/agent-slice';

const AGENT_TYPE_COLORS: Record<string, { bg: string; text: string; border: string }> = {
  claude_code: {
    bg: 'bg-blue-500/10',
    text: 'text-blue-400',
    border: 'border-blue-500/20',
  },
  cursor: {
    bg: 'bg-purple-500/10',
    text: 'text-purple-400',
    border: 'border-purple-500/20',
  },
};

const DEFAULT_AGENT_STYLE = {
  bg: 'bg-gray-500/10',
  text: 'text-gray-400',
  border: 'border-gray-500/20',
};

const MEMORY_TYPE_ICONS: Record<string, string> = {
  discovery: 'D',
  decision: 'R',
  context: 'C',
  warning: '!',
  preference: 'P',
};

const AGENT_TYPE_OPTIONS = ['All', 'claude_code', 'cursor'] as const;
const MEMORY_TYPE_OPTIONS = ['All', 'discovery', 'decision', 'context', 'warning', 'preference'] as const;

function isExpired(entry: AgentMemoryEntry): boolean {
  if (!entry.expires_at) return false;
  return new Date(entry.expires_at) < new Date();
}

function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

export const AgentMemoryPanel = memo(function AgentMemoryPanel() {
  const { t } = useTranslation();
  const [agentFilter, setAgentFilter] = useState<string>('All');
  const [memoryFilter, setMemoryFilter] = useState<string>('All');
  const [expandedId, setExpandedId] = useState<number | null>(null);

  const { agentMemories, agentDataExists, agentMemoryLoading } = useAppStore(
    useShallow((s) => ({
      agentMemories: s.agentMemories,
      agentDataExists: s.agentDataExists,
      agentMemoryLoading: s.agentMemoryLoading,
    })),
  );

  const loadAgentMemories = useAppStore((s) => s.loadAgentMemories);
  const checkAgentDataExists = useAppStore((s) => s.checkAgentDataExists);
  const promoteMemoryToDecision = useAppStore((s) => s.promoteMemoryToDecision);

  useEffect(() => {
    checkAgentDataExists();
  }, [checkAgentDataExists]);

  useEffect(() => {
    if (agentDataExists) {
      loadAgentMemories();
    }
  }, [agentDataExists, loadAgentMemories]);

  const filtered = useMemo(() => {
    return agentMemories.filter((m) => {
      if (agentFilter !== 'All' && m.agent_type !== agentFilter) return false;
      if (memoryFilter !== 'All' && m.memory_type !== memoryFilter) return false;
      return true;
    });
  }, [agentMemories, agentFilter, memoryFilter]);

  if (!agentDataExists && !agentMemoryLoading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
        <div className="px-5 py-4 border-b border-border flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-sm text-text-muted">A</span>
          </div>
          <h2 className="font-medium text-white text-sm">{t('agentMemory.title')}</h2>
        </div>
        <div className="p-8 text-center">
          <div className="text-sm text-text-secondary">{t('agentMemory.empty')}</div>
          <div className="text-xs text-text-muted mt-1">
            {t('agentMemory.emptyHint')}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-sm text-text-muted">A</span>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('agentMemory.title')}</h2>
            <p className="text-xs text-text-muted">
              {t('agentMemory.count', { count: agentMemories.length })}
            </p>
          </div>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="px-5 py-3 border-b border-border flex items-center gap-3">
        <select
          value={agentFilter}
          onChange={(e) => setAgentFilter(e.target.value)}
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-white border border-border rounded-lg focus:outline-none focus:border-white/30"
        >
          {AGENT_TYPE_OPTIONS.map((opt) => (
            <option key={opt} value={opt}>
              {opt === 'All' ? t('agentMemory.allAgents') : opt}
            </option>
          ))}
        </select>
        <select
          value={memoryFilter}
          onChange={(e) => setMemoryFilter(e.target.value)}
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-white border border-border rounded-lg focus:outline-none focus:border-white/30"
        >
          {MEMORY_TYPE_OPTIONS.map((opt) => (
            <option key={opt} value={opt}>
              {opt === 'All' ? t('agentMemory.allTypes') : opt}
            </option>
          ))}
        </select>
        <span className="text-[10px] text-text-muted ml-auto">
          {t('agentMemory.results', { count: filtered.length })}
        </span>
      </div>

      {/* Loading */}
      {agentMemoryLoading && (
        <div className="p-4 text-xs text-text-muted text-center">{t('agentMemory.loading')}</div>
      )}

      {/* Empty filtered state */}
      {!agentMemoryLoading && filtered.length === 0 && (
        <div className="p-8 text-center">
          <div className="text-sm text-text-secondary">{t('agentMemory.noMatch')}</div>
          <div className="text-xs text-text-muted mt-1">{t('agentMemory.noMatchHint')}</div>
        </div>
      )}

      {/* Timeline */}
      {!agentMemoryLoading && filtered.length > 0 && (
        <div className="p-3 space-y-2">
          {filtered.map((m) => {
            const expired = isExpired(m);
            const agentStyle = AGENT_TYPE_COLORS[m.agent_type] || DEFAULT_AGENT_STYLE;
            const memoryIcon = MEMORY_TYPE_ICONS[m.memory_type] || '?';
            const isExpanded = expandedId === m.id;

            return (
              <div
                key={m.id}
                className={`rounded-lg border border-border bg-bg-tertiary/50 transition-all ${
                  expired ? 'opacity-50' : ''
                }`}
              >
                <button
                  onClick={() => setExpandedId(isExpanded ? null : m.id)}
                  className="w-full px-4 py-3 flex items-center gap-3 text-left"
                >
                  {/* Memory type icon */}
                  <div className="w-6 h-6 bg-bg-primary rounded flex items-center justify-center flex-shrink-0">
                    <span className="text-[10px] font-mono text-text-secondary">{memoryIcon}</span>
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm text-white font-medium truncate">{m.subject}</span>
                      {expired && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-500/10 text-gray-500 border border-gray-500/20">
                          {t('agentMemory.expired')}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-text-secondary mt-0.5 truncate">{m.content}</p>
                  </div>

                  {/* Agent badge + timestamp */}
                  <div className="flex items-center gap-2 flex-shrink-0">
                    <span
                      className={`text-[10px] px-1.5 py-0.5 rounded ${agentStyle.bg} ${agentStyle.text} border ${agentStyle.border}`}
                    >
                      {m.agent_type}
                    </span>
                    <span className="text-[10px] text-text-muted font-mono">
                      {formatTimestamp(m.created_at)}
                    </span>
                    <span className="text-text-muted text-xs">{isExpanded ? '\u25BE' : '\u25B8'}</span>
                  </div>
                </button>

                {isExpanded && (
                  <div className="px-4 pb-3 border-t border-border/50 space-y-3">
                    {/* Full content */}
                    <div className="mt-3">
                      <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                        {t('agentMemory.content')}
                      </div>
                      <p className="text-xs text-text-secondary whitespace-pre-wrap">{m.content}</p>
                    </div>

                    {/* Context tags */}
                    {m.context_tags.length > 0 && (
                      <div>
                        <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                          {t('agentMemory.tags')}
                        </div>
                        <div className="flex flex-wrap gap-1.5">
                          {m.context_tags.map((tag, i) => (
                            <span
                              key={i}
                              className="text-[10px] px-2 py-0.5 bg-bg-secondary text-text-secondary border border-border rounded"
                            >
                              {tag}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Metadata */}
                    <div className="flex items-center gap-3 text-[10px] text-text-muted">
                      <span>{t('agentMemory.session')}: <span className="font-mono">{m.session_id.slice(0, 8)}</span></span>
                      <span>{t('agentMemory.type')}: {m.memory_type}</span>
                      <span>{t('agentMemory.created')} {new Date(m.created_at).toLocaleDateString()}</span>
                      {m.expires_at && (
                        <span>{t('agentMemory.expires')} {new Date(m.expires_at).toLocaleDateString()}</span>
                      )}
                      {m.promoted_to_decision_id !== null && (
                        <span className="text-green-400/70">
                          {t('agentMemory.promotedTo', { id: m.promoted_to_decision_id })}
                        </span>
                      )}
                    </div>

                    {/* Promote action for decision-type memories */}
                    {m.memory_type === 'decision' && m.promoted_to_decision_id === null && !expired && (
                      <div>
                        <button
                          onClick={() => promoteMemoryToDecision(m.id)}
                          className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded hover:bg-green-500/20 transition-colors"
                        >
                          {t('agentMemory.promoteToDecision')}
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});
