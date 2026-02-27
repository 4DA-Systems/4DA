import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface DeveloperDecision {
  id: number;
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string | null;
  alternatives_rejected: string[];
  context_tags: string[];
  confidence: number;
  status: string;
  superseded_by: number | null;
  created_at: string;
  updated_at: string;
}

const TYPES = ['All', 'tech_choice', 'architecture', 'workflow', 'pattern', 'dependency'] as const;
const STATUSES = ['All', 'active', 'superseded', 'reconsidering'] as const;

const TYPE_LABELS: Record<string, string> = {
  tech_choice: 'Tech Choice',
  architecture: 'Architecture',
  workflow: 'Workflow',
  pattern: 'Pattern',
  dependency: 'Dependency',
};

const TYPE_COLORS: Record<string, string> = {
  tech_choice: '#22C55E',
  architecture: '#D4AF37',
  workflow: '#3B82F6',
  pattern: '#A855F7',
  dependency: '#F97316',
};

const Pill = ({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) => (
  <button onClick={onClick} className={`px-2.5 py-1 text-xs rounded-full transition-all ${active ? 'bg-white text-[#0A0A0A] font-medium' : 'bg-bg-tertiary text-text-secondary border border-border hover:border-white/20'}`}>{label}</button>
);

const confColor = (c: number) => c >= 80 ? '#22C55E' : c >= 50 ? '#D4AF37' : '#EF4444';
const fmtDate = (iso: string) => { try { return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }); } catch { return iso; } };

export default function DecisionLog() {
  const { t } = useTranslation();
  const [decisions, setDecisions] = useState<DeveloperDecision[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filterType, setFilterType] = useState('All');
  const [filterStatus, setFilterStatus] = useState('All');
  const [expanded, setExpanded] = useState<number | null>(null);
  const [showForm, setShowForm] = useState(false);

  // Form state
  const [subject, setSubject] = useState('');
  const [decision, setDecision] = useState('');
  const [rationale, setRationale] = useState('');
  const [decType, setDecType] = useState('tech_choice');
  const [confidence, setConfidence] = useState(80);
  const [submitting, setSubmitting] = useState(false);

  const fetchDecisions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params: Record<string, unknown> = { limit: 100 };
      if (filterType !== 'All') params.decisionType = filterType;
      if (filterStatus !== 'All') params.status = filterStatus;
      const result = await invoke<DeveloperDecision[]>('get_decisions', params);
      setDecisions(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [filterType, filterStatus]);

  useEffect(() => {
    fetchDecisions();
  }, [fetchDecisions]);

  const addDecision = useCallback(async () => {
    if (!subject.trim() || !decision.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await invoke('record_developer_decision', {
        decisionType: decType,
        subject: subject.trim(),
        decision: decision.trim(),
        rationale: rationale.trim() || null,
        alternativesRejected: [],
        contextTags: [],
        confidence: confidence / 100,
      });
      // Reset form
      setSubject('');
      setDecision('');
      setRationale('');
      setDecType('tech_choice');
      setConfidence(80);
      setShowForm(false);
      await fetchDecisions();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [subject, decision, rationale, decType, confidence, fetchDecisions]);

  return (
    <div className="space-y-4">
      {/* Filters */}
      <div className="space-y-2">
        <div className="flex items-center gap-1.5 flex-wrap">
          <span className="text-xs text-[#666] mr-1">{t('toolkit.decisionLog.type')}:</span>
          {TYPES.map((t) => (
            <Pill
              key={t}
              label={t === 'All' ? 'All' : TYPE_LABELS[t] || t}
              active={filterType === t}
              onClick={() => setFilterType(t)}
            />
          ))}
        </div>
        <div className="flex items-center gap-1.5 flex-wrap">
          <span className="text-xs text-[#666] mr-1">{t('toolkit.decisionLog.status')}:</span>
          {STATUSES.map((s) => (
            <Pill
              key={s}
              label={s === 'All' ? 'All' : s.charAt(0).toUpperCase() + s.slice(1)}
              active={filterStatus === s}
              onClick={() => setFilterStatus(s)}
            />
          ))}
        </div>
      </div>

      {/* Add button / form toggle */}
      <div className="flex items-center gap-3">
        <button
          onClick={() => setShowForm((v) => !v)}
          className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            {showForm ? <path d="M5 12h14" /> : <><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></>}
          </svg>
          {showForm ? t('action.cancel') : t('toolkit.decisionLog.addDecision')}
        </button>
        {loading && (
          <div className="flex items-center gap-2 text-xs text-text-secondary">
            <div className="w-3 h-3 border-2 border-[#666] border-t-white rounded-full animate-spin" />
            {t('action.loading')}
          </div>
        )}
      </div>

      {/* Add form */}
      {showForm && (
        <div className="bg-bg-secondary border border-border rounded-xl p-4 space-y-3">
          <div>
            <label className="block text-xs text-text-secondary mb-1.5">{t('toolkit.decisionLog.subject')} *</label>
            <input
              type="text"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder={t('toolkit.decisionLog.subjectPlaceholder')}
              className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 transition-colors"
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1.5">{t('toolkit.decisionLog.decision')} *</label>
            <textarea
              value={decision}
              onChange={(e) => setDecision(e.target.value)}
              placeholder={t('toolkit.decisionLog.decisionPlaceholder')}
              rows={2}
              className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 transition-colors resize-y"
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1.5">{t('toolkit.decisionLog.rationale')}</label>
            <textarea
              value={rationale}
              onChange={(e) => setRationale(e.target.value)}
              placeholder={t('toolkit.decisionLog.rationalePlaceholder')}
              rows={2}
              className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 transition-colors resize-y"
            />
          </div>
          <div className="flex items-end gap-4">
            <div className="flex-1 max-w-[180px]">
              <label className="block text-xs text-text-secondary mb-1.5">{t('toolkit.decisionLog.type')}</label>
              <select
                value={decType}
                onChange={(e) => setDecType(e.target.value)}
                className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded-lg text-white focus:outline-none focus:border-white/30 transition-colors"
              >
                {TYPES.filter((t) => t !== 'All').map((t) => (
                  <option key={t} value={t}>{TYPE_LABELS[t]}</option>
                ))}
              </select>
            </div>
            <div className="flex-1 max-w-[220px]">
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('toolkit.decisionLog.confidence')}: <span className="font-mono" style={{ color: confColor(confidence) }}>{confidence}%</span>
              </label>
              <input
                type="range"
                min={0}
                max={100}
                value={confidence}
                onChange={(e) => setConfidence(Number(e.target.value))}
                className="w-full accent-white"
              />
            </div>
            <button
              onClick={addDecision}
              disabled={submitting || !subject.trim() || !decision.trim()}
              className="px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {submitting ? t('toolkit.decisionLog.adding') : t('toolkit.decisionLog.addDecision')}
            </button>
          </div>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-[#EF4444]/60 hover:text-[#EF4444] text-xs">
            {t('action.dismiss')}
          </button>
        </div>
      )}

      {/* Decision cards */}
      {decisions.length > 0 ? (
        <div className="space-y-2">
          {decisions.map((d) => {
            const isExpanded = expanded === d.id;
            const typeColor = TYPE_COLORS[d.decision_type] || '#A0A0A0';
            return (
              <div
                key={d.id}
                onClick={() => setExpanded(isExpanded ? null : d.id)}
                className="bg-bg-secondary border border-border rounded-lg p-4 cursor-pointer hover:border-white/20 transition-all"
              >
                {/* Header row */}
                <div className="flex items-start gap-3 mb-1.5">
                  <h4 className="text-sm font-medium text-white flex-1 leading-snug">
                    {d.subject}
                  </h4>
                  <div className="flex items-center gap-1.5 shrink-0">
                    <span
                      className="px-2 py-0.5 text-[10px] font-medium rounded"
                      style={{ backgroundColor: `${typeColor}15`, color: typeColor, border: `1px solid ${typeColor}30` }}
                    >
                      {TYPE_LABELS[d.decision_type] || d.decision_type}
                    </span>
                    <span className={`px-2 py-0.5 text-[10px] rounded ${
                      d.status === 'active'
                        ? 'bg-[#22C55E]/10 text-[#22C55E] border border-[#22C55E]/20'
                        : d.status === 'superseded'
                          ? 'bg-[#666]/10 text-[#666] border border-[#666]/20'
                          : 'bg-[#D4AF37]/10 text-[#D4AF37] border border-[#D4AF37]/20'
                    }`}>
                      {d.status}
                    </span>
                  </div>
                </div>

                {/* Decision text (clamped) */}
                <p className={`text-xs text-text-secondary leading-relaxed ${isExpanded ? '' : 'line-clamp-2'}`}>
                  {d.decision}
                </p>

                {/* Confidence bar */}
                <div className="flex items-center gap-2 mt-2">
                  <div className="flex-1 h-1 bg-white/10 rounded-full overflow-hidden max-w-[120px]">
                    <div
                      className="h-full rounded-full transition-all"
                      style={{ width: `${Math.round(d.confidence * 100)}%`, backgroundColor: confColor(d.confidence * 100) }}
                    />
                  </div>
                  <span className="text-[10px] font-mono" style={{ color: confColor(d.confidence * 100) }}>
                    {Math.round(d.confidence * 100)}%
                  </span>
                  <span className="text-xs text-[#666] ml-auto">{fmtDate(d.created_at)}</span>
                </div>

                {/* Expanded details */}
                {isExpanded && (
                  <div className="mt-3 pt-3 border-t border-border space-y-2">
                    {d.rationale && (
                      <div>
                        <span className="text-[10px] text-[#666] uppercase tracking-wider">{t('toolkit.decisionLog.rationale')}</span>
                        <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">{d.rationale}</p>
                      </div>
                    )}
                    {d.alternatives_rejected.length > 0 && (
                      <div>
                        <span className="text-[10px] text-[#666] uppercase tracking-wider">{t('toolkit.decisionLog.alternativesRejected')}</span>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {d.alternatives_rejected.map((alt, i) => (
                            <span key={i} className="px-2 py-0.5 text-xs text-text-secondary bg-bg-tertiary border border-border rounded-full">
                              {alt}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                    {d.context_tags.length > 0 && (
                      <div>
                        <span className="text-[10px] text-[#666] uppercase tracking-wider">{t('toolkit.decisionLog.tags')}</span>
                        <div className="flex flex-wrap gap-1 mt-1">
                          {d.context_tags.map((tag) => (
                            <span key={tag} className="px-2 py-0.5 text-xs text-[#D4AF37] bg-[#D4AF37]/10 border border-[#D4AF37]/20 rounded-full">
                              {tag}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ) : !loading ? (
        <div className="flex flex-col items-center justify-center py-14 text-center">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="#666" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3">
            <path d="M12 3v18" /><path d="M5 6l7-3 7 3" /><path d="M5 6v12l7 3 7-3V6" />
          </svg>
          <p className="text-sm text-text-secondary mb-1">{t('toolkit.decisionLog.empty')}</p>
          <p className="text-xs text-[#666]">
            {t('toolkit.decisionLog.emptyHint')}
          </p>
        </div>
      ) : null}
    </div>
  );
}
