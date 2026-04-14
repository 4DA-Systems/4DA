import { useState, useEffect, memo, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { useFourdaComponent } from '../hooks/use-fourda-component';
import type { DeveloperDecision } from '../store/decisions-slice';
import { DECISION_TYPES, STATUS_STYLES, EMPTY_FORM } from './decision-memory-constants';
import type { NewDecisionForm } from './decision-memory-constants';
import { formatLocalDate } from '../utils/format-date';

export const DecisionMemory = memo(function DecisionMemory() {
  const { t } = useTranslation();
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<NewDecisionForm>({ ...EMPTY_FORM });
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Data selectors (may change, use useShallow)
  const { decisions, decisionsLoading, decisionsError } = useAppStore(
    useShallow((s) => ({
      decisions: s.decisions,
      decisionsLoading: s.decisionsLoading,
      decisionsError: s.decisionsError,
    })),
  );

  // Action selectors (stable references, no need for useShallow)
  const loadDecisions = useAppStore((s) => s.loadDecisions);
  const recordDecision = useAppStore((s) => s.recordDecision);
  const updateDecision = useAppStore((s) => s.updateDecision);
  const removeTechDecision = useAppStore((s) => s.removeTechDecision);
  const addToast = useAppStore((s) => s.addToast);

  const { containerRef: tetRef, elementRef: tetEl } = useFourdaComponent('fourda-tetrahedron');

  useEffect(() => {
    loadDecisions();
  }, [loadDecisions]);

  // Tetrahedron responds to decision state: more reconsidering = faster spin, dimmer glow
  useEffect(() => {
    const reconsidering = decisions.filter(d => d.status === 'reconsidering').length;
    const total = decisions.length || 1;
    const stability = 1.0 - reconsidering / total;
    tetEl.current?.setParam?.('rotation_speed', 0.2 + (1.0 - stability) * 0.8);
    tetEl.current?.setParam?.('glow_intensity', 0.5 + stability * 0.5);
  }, [decisions, tetEl]);

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
          <div ref={tetRef} className="w-12 h-12 rounded-lg overflow-hidden border border-border/20" aria-hidden="true" />
          <div>
            <h2 className="font-medium text-white text-sm">{t('decisions.title')}</h2>
            <p className="text-xs text-text-muted">
              {t('decisions.recorded', { count: decisions.length })}
            </p>
          </div>
        </div>
        <button
          onClick={() => setShowForm(!showForm)}
          aria-expanded={showForm}
          aria-label={showForm ? t('action.cancel') : t('decisions.record')}
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded hover:border-white/20 transition-colors"
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
              aria-label={t('decisions.typeLabel', 'Decision type')}
              className="px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg focus:outline-none focus:border-white/30"
            >
              {DECISION_TYPES.map((dtype) => (
                <option key={dtype} value={dtype}>
                  {t(`decisions.type.${dtype}`)}
                </option>
              ))}
            </select>
            <input
              type="text"
              placeholder={t('decisions.subject')}
              aria-label={t('decisions.subject')}
              value={form.subject}
              onChange={(e) => setForm({ ...form, subject: e.target.value })}
              className="flex-1 px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30"
            />
          </div>
          <textarea
            placeholder={t('decisions.whatDecided')}
            aria-label={t('decisions.whatDecided')}
            value={form.decision}
            onChange={(e) => setForm({ ...form, decision: e.target.value })}
            rows={2}
            aria-required="true"
            className="w-full px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30 resize-none"
          />
          <textarea
            placeholder={t('decisions.rationaleOptional')}
            aria-label={t('decisions.rationaleOptional')}
            value={form.rationale}
            onChange={(e) => setForm({ ...form, rationale: e.target.value })}
            rows={2}
            className="w-full px-3 py-2 text-xs bg-bg-tertiary text-white border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-white/30 resize-none"
          />
          <div className="flex items-center gap-3">
            <label className="text-xs text-text-muted">
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
              aria-label={t('decisions.confidence', { value: Math.round(form.confidence * 100) })}
              className="flex-1 accent-white h-1"
            />
            <button
              onClick={handleSubmit}
              disabled={isSubmitting || !form.subject.trim() || !form.decision.trim()}
              className="px-4 py-2 text-xs bg-white text-black rounded-lg font-medium hover:bg-gray-200 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
              aria-label={t('decisions.saveDecision', 'Save decision')}
            >
              {t('action.save')}
            </button>
          </div>
        </div>
      )}

      {/* Auto-detected tech notice */}
      {!decisionsLoading && decisions.some(d => d.decision_type === 'tech_choice' && d.rationale === 'Inferred from project setup') && (
        <div className="px-5 py-3 border-b border-border bg-amber-500/5 flex items-start gap-3">
          <span className="text-amber-400 text-xs mt-0.5 flex-shrink-0" aria-hidden="true">!</span>
          <div className="flex-1 min-w-0">
            <p className="text-xs text-amber-400 font-medium">{t('decisions.autoDetectedNotice')}</p>
            <p className="text-[10px] text-text-muted mt-0.5">{t('decisions.autoDetectedHint')}</p>
          </div>
        </div>
      )}

      {/* Decisions list (live region) */}
      <div aria-live="polite">
      {/* Error */}
      {decisionsError && !decisionsLoading && (
        <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
          <p className="text-text-secondary text-sm">{t('error.generic')}</p>
          <button
            onClick={loadDecisions}
            className="px-3 py-1.5 text-xs bg-bg-tertiary hover:bg-white/10 rounded transition-colors text-text-secondary"
            aria-label={t('decisions.retryLoad', 'Retry loading decisions')}
          >
            {t('action.retry')}
          </button>
        </div>
      )}

      {/* Loading */}
      {decisionsLoading && (
        <div className="p-4 space-y-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="rounded-lg border border-border bg-bg-tertiary/50 p-4">
              <div className="h-4 w-32 bg-bg-tertiary rounded animate-pulse mb-2" />
              <div className="h-3 w-56 bg-bg-tertiary rounded animate-pulse" />
            </div>
          ))}
        </div>
      )}

      {/* Empty State */}
      {!decisionsLoading && !decisionsError && decisions.length === 0 && (
        <div className="p-8 text-center">
          <div className="text-sm text-text-muted">{t('decisions.noDecisions')}</div>
          <div className="text-xs text-text-muted mt-1">
            {t('decisions.noDecisionsHint')}
          </div>
        </div>
      )}

      {/* Grouped Decisions */}
      {!decisionsLoading && !decisionsError && Object.entries(grouped).map(([type, items]) => (
        <div key={type} className="border-b border-border last:border-b-0">
          <div className="px-5 py-2.5 bg-bg-primary/50">
            <span className="text-[10px] font-medium text-text-muted uppercase tracking-wider">
              {t(`decisions.type.${type}`)}
            </span>
            <span className="text-[10px] text-text-muted ms-2">{items.length}</span>
          </div>
          <div className="p-3 space-y-2">
            {items.map((d) => {
              const isExpanded = expandedId === d.id;
              const status = (STATUS_STYLES[d.status] ?? STATUS_STYLES.active)!;

              return (
                <div
                  key={d.id}
                  className="rounded-lg border border-border bg-bg-tertiary/50 transition-all"
                >
                  <button
                    onClick={() => setExpandedId(isExpanded ? null : d.id)}
                    className="w-full px-4 py-3 flex items-center gap-3 text-start"
                    aria-expanded={isExpanded}
                    aria-label={t('decisions.toggleDetail', `${isExpanded ? 'Collapse' : 'Expand'} decision: ${d.subject}`)}
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
                      <p className="text-xs text-text-secondary mt-0.5 truncate">{d.decision}</p>
                    </div>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <span className="text-[10px] text-text-muted font-mono">
                        {Math.round(d.confidence * 100)}%
                      </span>
                      <span className="text-text-muted text-xs" aria-hidden="true">
                        {isExpanded ? '\u25BE' : '\u25B8'}
                      </span>
                    </div>
                  </button>

                  {isExpanded && (
                    <div className="px-4 pb-3 border-t border-border/50 space-y-3">
                      {/* Decision text */}
                      <div className="mt-3">
                        <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                          {t('decisions.decision')}
                        </div>
                        <p className="text-xs text-text-secondary">{d.decision}</p>
                      </div>

                      {/* Rationale */}
                      {d.rationale && (
                        <div>
                          <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                            {t('decisions.rationale')}
                          </div>
                          <p className="text-xs text-text-secondary">{d.rationale}</p>
                        </div>
                      )}

                      {/* Alternatives rejected */}
                      {d.alternatives_rejected.length > 0 && (
                        <div>
                          <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
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
                          <div className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
                            {t('decisions.tags')}
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {d.context_tags.map((tag, i) => (
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

                      {/* Metadata row */}
                      <div className="flex items-center gap-3 text-[10px] text-text-muted">
                        <span>{t('decisions.createdDate', { date: formatLocalDate(d.created_at) })}</span>
                        {d.updated_at !== d.created_at && (
                          <span>{t('decisions.updatedDate', { date: formatLocalDate(d.updated_at) })}</span>
                        )}
                        {d.superseded_by !== null && (
                          <span className="text-amber-400/70">
                            {t('decisions.supersededBy', { id: d.superseded_by })}
                          </span>
                        )}
                      </div>

                      {/* Actions */}
                      {d.status === 'active' && (
                        <div className="flex gap-2">
                          <button
                            onClick={() => handleReconsider(d.id)}
                            className="px-3 py-1.5 text-xs bg-amber-500/10 text-amber-400 border border-amber-500/20 rounded hover:bg-amber-500/20 transition-colors"
                            aria-label={`${t('decisions.reconsider')} ${d.subject}`}
                          >
                            {t('decisions.reconsider')}
                          </button>
                          <button
                            onClick={() => handleSupersede(d.id)}
                            className="px-3 py-1.5 text-xs bg-gray-500/10 text-text-secondary border border-gray-500/20 rounded hover:bg-gray-500/20 transition-colors"
                            aria-label={`${t('decisions.supersede')} ${d.subject}`}
                          >
                            {t('decisions.supersede')}
                          </button>
                          {d.decision_type === 'tech_choice' && (
                            <button
                              onClick={() => {
                                if (window.confirm('Remove this decision? This cannot be undone.')) {
                                  removeTechDecision(d.subject);
                                }
                              }}
                              className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/20 rounded hover:bg-red-500/20 transition-colors"
                              aria-label={`${t('decisions.remove')} ${d.subject}`}
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
                            aria-label={`${t('decisions.reaffirm')} ${d.subject}`}
                          >
                            {t('decisions.reaffirm')}
                          </button>
                          <button
                            onClick={() => handleSupersede(d.id)}
                            className="px-3 py-1.5 text-xs bg-gray-500/10 text-text-secondary border border-gray-500/20 rounded hover:bg-gray-500/20 transition-colors"
                            aria-label={`${t('decisions.supersede')} ${d.subject}`}
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
    </div>
  );
});
