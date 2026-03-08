import { useState, useEffect, memo, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import type { DeveloperDecision } from '../store/decisions-slice';

const DECISION_TYPES = [
  'tech_choice',
  'architecture',
  'workflow',
  'pattern',
  'dependency',
] as const;

const TYPE_LABELS: Record<string, string> = {
  tech_choice: 'Tech Choice',
  architecture: 'Architecture',
  workflow: 'Workflow',
  pattern: 'Pattern',
  dependency: 'Dependency',
};

const STATUS_STYLES: Record<string, { text: string; bg: string; border: string }> = {
  active: {
    text: 'text-green-400',
    bg: 'bg-green-500/10',
    border: 'border-green-500/20',
  },
  reconsidering: {
    text: 'text-amber-400',
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/20',
  },
  superseded: {
    text: 'text-gray-500',
    bg: 'bg-gray-500/10',
    border: 'border-gray-500/20',
  },
};

interface NewDecisionForm {
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string;
  confidence: number;
}

const EMPTY_FORM: NewDecisionForm = {
  decision_type: 'tech_choice',
  subject: '',
  decision: '',
  rationale: '',
  confidence: 0.8,
};

export const DecisionMemory = memo(function DecisionMemory() {
  const { t } = useTranslation();
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<NewDecisionForm>({ ...EMPTY_FORM });
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Data selectors (may change, use useShallow)
  const { decisions, decisionsLoading } = useAppStore(
    useShallow((s) => ({
      decisions: s.decisions,
      decisionsLoading: s.decisionsLoading,
    })),
  );

  // Action selectors (stable references, no need for useShallow)
  const loadDecisions = useAppStore((s) => s.loadDecisions);
  const recordDecision = useAppStore((s) => s.recordDecision);
  const updateDecision = useAppStore((s) => s.updateDecision);
  const removeTechDecision = useAppStore((s) => s.removeTechDecision);
  const addToast = useAppStore((s) => s.addToast);

  useEffect(() => {
    loadDecisions();
  }, [loadDecisions]);

  const grouped = useMemo(() =>
    DECISION_TYPES.reduce<Record<string, DeveloperDecision[]>>(
      (acc, type) => {
        const items = decisions.filter((d) => d.decision_type === type);
        if (items.length > 0) acc[type] = items;
        return acc;
      },
      {},
    ),
  [decisions]);

  const handleSubmit = useCallback(async () => {
    if (!form.subject.trim() || !form.decision.trim()) return;
    setIsSubmitting(true);
    try {
      await recordDecision({
        decision_type: form.decision_type,
        subject: form.subject.trim(),
        decision: form.decision.trim(),
        rationale: form.rationale.trim() || undefined,
        confidence: form.confidence,
      });
      setForm({ ...EMPTY_FORM });
      setShowForm(false);
    } catch {
      addToast('error', t('error.generic'));
    } finally {
      setIsSubmitting(false);
    }
  }, [form, recordDecision, addToast, t]);

  const handleSupersede = useCallback(async (id: number) => {
    setIsSubmitting(true);
    try {
      await updateDecision(id, { status: 'superseded' });
    } catch {
      addToast('error', t('error.generic'));
    } finally {
      setIsSubmitting(false);
    }
  }, [updateDecision, addToast, t]);

  const handleReconsider = useCallback(async (id: number) => {
    setIsSubmitting(true);
    try {
      await updateDecision(id, { status: 'reconsidering' });
    } catch {
      addToast('error', t('error.generic'));
    } finally {
      setIsSubmitting(false);
    }
  }, [updateDecision, addToast, t]);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-sm text-gray-400">D</span>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('decisions.title')}</h2>
            <p className="text-xs text-gray-500">
              {t('decisions.recorded', { count: decisions.length })}
            </p>
          </div>
        </div>
        <button
          onClick={() => setShowForm(!showForm)}
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-gray-300 border border-border rounded hover:border-white/20 transition-colors"
        >
          {showForm ? t('action.cancel') : t('decisions.record')}
        </button>
      </div>

      {/* Inline Form */}
      {showForm && (
        <div role="form" aria-label={t('decisions.record')} aria-busy={isSubmitting} className="p-4 border-b border-border space-y-3">
          <div className="flex gap-3">
            <select
              value={form.decision_type}
              onChange={(e) => setForm({ ...form, decision_type: e.target.value })}
              className="px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg focus:outline-none focus:border-white/30"
            >
              {DECISION_TYPES.map((t) => (
                <option key={t} value={t}>
                  {TYPE_LABELS[t]}
                </option>
              ))}
            </select>
            <input
              type="text"
              placeholder={t('decisions.subject')}
              value={form.subject}
              onChange={(e) => setForm({ ...form, subject: e.target.value })}
              className="flex-1 px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30"
            />
          </div>
          <textarea
            placeholder={t('decisions.whatDecided')}
            value={form.decision}
            onChange={(e) => setForm({ ...form, decision: e.target.value })}
            rows={2}
            aria-required="true"
            className="w-full px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30 resize-none"
          />
          <textarea
            placeholder={t('decisions.rationaleOptional')}
            value={form.rationale}
            onChange={(e) => setForm({ ...form, rationale: e.target.value })}
            rows={2}
            className="w-full px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30 resize-none"
          />
          <div className="flex items-center gap-3">
            <label className="text-xs text-gray-500">
              {t('decisions.confidence', { value: Math.round(form.confidence * 100) })}
            </label>
            <input
              type="range"
              min={0}
              max={100}
              value={Math.round(form.confidence * 100)}
              onChange={(e) =>
                setForm({ ...form, confidence: parseInt(e.target.value, 10) / 100 })
              }
              className="flex-1 accent-white h-1"
            />
            <button
              onClick={handleSubmit}
              disabled={isSubmitting || !form.subject.trim() || !form.decision.trim()}
              className="px-4 py-2 text-xs bg-white text-black rounded-lg font-medium hover:bg-gray-200 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
            >
              {t('action.save')}
            </button>
          </div>
        </div>
      )}

      {/* Auto-detected tech notice */}
      {!decisionsLoading && decisions.some(d => d.decision_type === 'tech_choice' && d.rationale === 'Inferred from project setup') && (
        <div className="px-5 py-3 border-b border-border bg-amber-500/5 flex items-start gap-3">
          <span className="text-amber-400 text-xs mt-0.5 flex-shrink-0">!</span>
          <div className="flex-1 min-w-0">
            <p className="text-xs text-amber-400 font-medium">{t('decisions.autoDetectedNotice')}</p>
            <p className="text-[10px] text-gray-500 mt-0.5">{t('decisions.autoDetectedHint')}</p>
          </div>
        </div>
      )}

      {/* Decisions list (live region) */}
      <div aria-live="polite">
      {/* Loading */}
      {decisionsLoading && (
        <div className="p-4 text-xs text-gray-500 text-center">{t('decisions.loading')}</div>
      )}

      {/* Empty State */}
      {!decisionsLoading && decisions.length === 0 && (
        <div className="p-8 text-center">
          <div className="text-sm text-gray-500">{t('decisions.noDecisions')}</div>
          <div className="text-xs text-gray-600 mt-1">
            {t('decisions.noDecisionsHint')}
          </div>
        </div>
      )}

      {/* Grouped Decisions */}
      {!decisionsLoading && Object.entries(grouped).map(([type, items]) => (
        <div key={type} className="border-b border-border last:border-b-0">
          <div className="px-5 py-2.5 bg-bg-primary/50">
            <span className="text-[10px] font-medium text-gray-500 uppercase tracking-wider">
              {TYPE_LABELS[type] || type}
            </span>
            <span className="text-[10px] text-gray-600 ml-2">{items.length}</span>
          </div>
          <div className="p-3 space-y-2">
            {items.map((d) => {
              const isExpanded = expandedId === d.id;
              const status = STATUS_STYLES[d.status] || STATUS_STYLES.active;

              return (
                <div
                  key={d.id}
                  className="rounded-lg border border-border bg-bg-tertiary/50 transition-all"
                >
                  <button
                    onClick={() => setExpandedId(isExpanded ? null : d.id)}
                    className="w-full px-4 py-3 flex items-center gap-3 text-left"
                  >
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm text-white font-medium truncate">
                          {d.subject}
                        </span>
                        <span
                          className={`text-[10px] px-1.5 py-0.5 rounded ${status.bg} ${status.text} border ${status.border}`}
                        >
                          {d.status}
                        </span>
                      </div>
                      <p className="text-xs text-gray-400 mt-0.5 truncate">{d.decision}</p>
                    </div>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <span className="text-[10px] text-gray-600 font-mono">
                        {Math.round(d.confidence * 100)}%
                      </span>
                      <span className="text-gray-500 text-xs">
                        {isExpanded ? '\u25BE' : '\u25B8'}
                      </span>
                    </div>
                  </button>

                  {isExpanded && (
                    <div className="px-4 pb-3 border-t border-border/50 space-y-3">
                      {/* Decision text */}
                      <div className="mt-3">
                        <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
                          {t('decisions.decision')}
                        </div>
                        <p className="text-xs text-gray-300">{d.decision}</p>
                      </div>

                      {/* Rationale */}
                      {d.rationale && (
                        <div>
                          <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
                            {t('decisions.rationale')}
                          </div>
                          <p className="text-xs text-gray-400">{d.rationale}</p>
                        </div>
                      )}

                      {/* Alternatives rejected */}
                      {d.alternatives_rejected.length > 0 && (
                        <div>
                          <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
                            {t('decisions.alternativesRejected')}
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {d.alternatives_rejected.map((alt, i) => (
                              <span
                                key={i}
                                className="text-[10px] px-2 py-0.5 bg-red-500/10 text-red-400 border border-red-500/20 rounded"
                              >
                                {alt}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Context tags */}
                      {d.context_tags.length > 0 && (
                        <div>
                          <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
                            {t('decisions.tags')}
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {d.context_tags.map((tag, i) => (
                              <span
                                key={i}
                                className="text-[10px] px-2 py-0.5 bg-bg-secondary text-gray-400 border border-border rounded"
                              >
                                {tag}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Metadata row */}
                      <div className="flex items-center gap-3 text-[10px] text-gray-600">
                        <span>Created {new Date(d.created_at).toLocaleDateString()}</span>
                        {d.updated_at !== d.created_at && (
                          <span>Updated {new Date(d.updated_at).toLocaleDateString()}</span>
                        )}
                        {d.superseded_by !== null && (
                          <span className="text-amber-400/70">
                            Superseded by #{d.superseded_by}
                          </span>
                        )}
                      </div>

                      {/* Actions */}
                      {d.status === 'active' && (
                        <div className="flex gap-2">
                          <button
                            onClick={() => handleReconsider(d.id)}
                            className="px-3 py-1.5 text-xs bg-amber-500/10 text-amber-400 border border-amber-500/20 rounded hover:bg-amber-500/20 transition-colors"
                          >
                            {t('decisions.reconsider')}
                          </button>
                          <button
                            onClick={() => handleSupersede(d.id)}
                            className="px-3 py-1.5 text-xs bg-gray-500/10 text-gray-400 border border-gray-500/20 rounded hover:bg-gray-500/20 transition-colors"
                          >
                            {t('decisions.supersede')}
                          </button>
                          {d.decision_type === 'tech_choice' && (
                            <button
                              onClick={() => removeTechDecision(d.subject)}
                              className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/20 rounded hover:bg-red-500/20 transition-colors"
                            >
                              {t('decisions.remove')}
                            </button>
                          )}
                        </div>
                      )}
                      {d.status === 'reconsidering' && (
                        <div className="flex gap-2">
                          <button
                            onClick={() => updateDecision(d.id, { status: 'active' })}
                            className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded hover:bg-green-500/20 transition-colors"
                          >
                            {t('decisions.reaffirm')}
                          </button>
                          <button
                            onClick={() => handleSupersede(d.id)}
                            className="px-3 py-1.5 text-xs bg-gray-500/10 text-gray-400 border border-gray-500/20 rounded hover:bg-gray-500/20 transition-colors"
                          >
                            {t('decisions.supersede')}
                          </button>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
});
