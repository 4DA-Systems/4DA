import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';

interface SovereignProfileProps {
  onGenerateDocument: (doc: string) => void;
}

const CATEGORY_META: Record<string, { label: string; icon: string; hint: string }> = {
  cpu: { label: 'CPU / Processor', icon: 'C', hint: 'Run lscpu or Get-CimInstance in Module S, Lesson 1' },
  ram: { label: 'Memory (RAM)', icon: 'R', hint: 'Run free -h in Module S, Lesson 1' },
  gpu: { label: 'GPU / Accelerator', icon: 'G', hint: 'Run nvidia-smi in Module S, Lesson 2' },
  storage: { label: 'Storage', icon: 'S', hint: 'Run df -h in Module S, Lesson 1' },
  network: { label: 'Network', icon: 'N', hint: 'Run speedtest in Module S, Lesson 3' },
  os: { label: 'Operating System', icon: 'O', hint: 'Run uname -a or systeminfo in Module S, Lesson 1' },
  llm: { label: 'LLM Infrastructure', icon: 'L', hint: 'Run ollama --version and ollama list in Module S, Lesson 4' },
  legal: { label: 'Legal Entity', icon: 'J', hint: 'Enter manually: business name, structure, jurisdiction' },
  budget: { label: 'Budget / Runway', icon: 'B', hint: 'Enter manually: monthly budget, runway months' },
};

const ALL_CATEGORIES = ['cpu', 'ram', 'gpu', 'storage', 'network', 'os', 'llm', 'legal', 'budget'];

function CompletenessRing({ percentage }: { percentage: number }) {
  const r = 20;
  const circ = 2 * Math.PI * r;
  const offset = circ - (percentage / 100) * circ;
  return (
    <svg width="52" height="52" viewBox="0 0 52 52" className="flex-shrink-0">
      <circle cx="26" cy="26" r={r} fill="none" stroke="#2A2A2A" strokeWidth="3" />
      <circle
        cx="26" cy="26" r={r} fill="none"
        stroke={percentage >= 100 ? '#22C55E' : '#D4AF37'}
        strokeWidth="3"
        strokeDasharray={circ}
        strokeDashoffset={offset}
        strokeLinecap="round"
        transform="rotate(-90 26 26)"
        className="transition-all duration-500"
      />
      <text x="26" y="27" textAnchor="middle" dominantBaseline="middle" fill="#A0A0A0" fontSize="11" fontFamily="Inter">
        {Math.round(percentage)}%
      </text>
    </svg>
  );
}

export function SovereignProfile({ onGenerateDocument }: SovereignProfileProps) {
  const { t } = useTranslation();
  const {
    sovereignProfile,
    profileCompleteness,
    profileLoading,
    generatedDocument,
  } = useAppStore(
    useShallow((s) => ({
      sovereignProfile: s.sovereignProfile,
      profileCompleteness: s.profileCompleteness,
      profileLoading: s.profileLoading,
      generatedDocument: s.generatedDocument,
    })),
  );

  const loadProfile = useAppStore((s) => s.loadSovereignProfile);
  const loadCompleteness = useAppStore((s) => s.loadProfileCompleteness);
  const saveFact = useAppStore((s) => s.saveFact);
  const generateDoc = useAppStore((s) => s.generateDocument);

  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [manualCategory, setManualCategory] = useState('legal');
  const [manualKey, setManualKey] = useState('');
  const [manualValue, setManualValue] = useState('');
  const [showDocument, setShowDocument] = useState(false);

  useEffect(() => {
    loadProfile();
    loadCompleteness();
  }, [loadProfile, loadCompleteness]);

  const toggleCategory = useCallback((cat: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  }, []);

  const handleSaveFact = useCallback(async () => {
    if (!manualKey.trim() || !manualValue.trim()) return;
    await saveFact(manualCategory, manualKey.trim(), manualValue.trim());
    setManualKey('');
    setManualValue('');
  }, [manualCategory, manualKey, manualValue, saveFact]);

  const handleGenerate = useCallback(async () => {
    await generateDoc();
    setShowDocument(true);
  }, [generateDoc]);

  // When document is generated, notify parent
  useEffect(() => {
    if (generatedDocument && showDocument) {
      onGenerateDocument(generatedDocument);
    }
  }, [generatedDocument, showDocument, onGenerateDocument]);

  // Group facts by category
  const factsByCategory: Record<string, Array<{ key: string; value: string; source_lesson: string | null; updated_at: string }>> = {};
  if (sovereignProfile) {
    for (const fact of sovereignProfile.facts) {
      if (!factsByCategory[fact.category]) factsByCategory[fact.category] = [];
      factsByCategory[fact.category].push(fact);
    }
  }

  if (profileLoading && !sovereignProfile) {
    return (
      <div className="bg-bg-secondary border border-border rounded-xl p-6 mt-4">
        <div className="flex items-center gap-3">
          <div className="w-5 h-5 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
          <span className="text-sm text-text-secondary">{t('playbook.sovereign.loading')}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary border border-border rounded-xl p-6 mt-4 space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-white">{t('playbook.sovereign.title')}</h3>
          <p className="text-xs text-[#666] mt-0.5">
            {t('playbook.sovereign.subtitle')}
          </p>
        </div>
        {profileCompleteness && (
          <CompletenessRing percentage={profileCompleteness.percentage} />
        )}
      </div>

      {/* Category Sections */}
      <div className="space-y-1">
        {ALL_CATEGORIES.map((cat) => {
          const meta = CATEGORY_META[cat];
          const facts = factsByCategory[cat];
          const hasFacts = facts && facts.length > 0;
          const isExpanded = expanded.has(cat);

          return (
            <div key={cat} className="border border-border rounded-lg overflow-hidden">
              <button
                onClick={() => toggleCategory(cat)}
                className="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-bg-tertiary transition-colors text-left"
              >
                <span
                  className={`w-6 h-6 rounded flex items-center justify-center text-[10px] font-bold flex-shrink-0 ${
                    hasFacts
                      ? 'bg-[#22C55E]/15 text-[#22C55E]'
                      : 'bg-bg-tertiary text-[#666]'
                  }`}
                >
                  {meta.icon}
                </span>
                <span className={`text-xs font-medium flex-1 ${hasFacts ? 'text-white' : 'text-[#666]'}`}>
                  {meta.label}
                </span>
                {hasFacts && (
                  <span className="text-[10px] text-text-secondary">{t('playbook.sovereign.factCount', { count: facts.length })}</span>
                )}
                <svg
                  width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#666" strokeWidth="2"
                  className={`transition-transform ${isExpanded ? 'rotate-180' : ''}`}
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>

              {isExpanded && (
                <div className="px-4 pb-3 pt-1 border-t border-border">
                  {hasFacts ? (
                    <div className="space-y-1.5">
                      {facts.map((fact, i) => (
                        <div key={i} className="flex items-start gap-2">
                          <span className="text-[10px] text-[#666] font-mono min-w-[80px] flex-shrink-0 pt-0.5">
                            {fact.key}
                          </span>
                          <span className="text-xs text-text-secondary font-mono break-all flex-1">
                            {fact.value}
                          </span>
                          {fact.source_lesson && fact.source_lesson !== 'manual' && (
                            <span className="text-[9px] text-[#D4AF37] bg-[#D4AF37]/10 px-1.5 py-0.5 rounded flex-shrink-0">
                              {fact.source_lesson}
                            </span>
                          )}
                          {fact.source_lesson === 'manual' && (
                            <span className="text-[9px] text-text-secondary bg-bg-tertiary px-1.5 py-0.5 rounded flex-shrink-0">
                              manual
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="text-[10px] text-[#666] italic">{meta.hint}</p>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Manual Entry Form */}
      <div className="border-t border-border pt-4">
        <p className="text-[10px] text-[#666] mb-2 uppercase tracking-wide font-medium">{t('playbook.sovereign.manualEntry')}</p>
        <div className="flex items-end gap-2">
          <div className="flex-shrink-0">
            <select
              value={manualCategory}
              onChange={(e) => setManualCategory(e.target.value)}
              className="bg-bg-primary border border-border rounded px-2 py-1.5 text-xs text-text-secondary focus:border-[#D4AF37] focus:outline-none"
            >
              {ALL_CATEGORIES.map((cat) => (
                <option key={cat} value={cat}>{CATEGORY_META[cat].label}</option>
              ))}
            </select>
          </div>
          <input
            type="text"
            value={manualKey}
            onChange={(e) => setManualKey(e.target.value)}
            placeholder={t('playbook.sovereign.keyPlaceholder')}
            className="bg-bg-primary border border-border rounded px-2 py-1.5 text-xs text-text-secondary w-28 focus:border-[#D4AF37] focus:outline-none font-mono"
          />
          <input
            type="text"
            value={manualValue}
            onChange={(e) => setManualValue(e.target.value)}
            placeholder={t('playbook.sovereign.valuePlaceholder')}
            className="bg-bg-primary border border-border rounded px-2 py-1.5 text-xs text-text-secondary flex-1 focus:border-[#D4AF37] focus:outline-none font-mono"
            onKeyDown={(e) => e.key === 'Enter' && handleSaveFact()}
          />
          <button
            onClick={handleSaveFact}
            disabled={!manualKey.trim() || !manualValue.trim()}
            className="px-3 py-1.5 text-xs font-medium bg-bg-tertiary text-text-secondary border border-border rounded hover:bg-[#2A2A2A] hover:text-white transition-colors disabled:opacity-40"
          >
            {t('action.save')}
          </button>
        </div>
      </div>

      {/* Generate Document Button */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleGenerate}
          className="px-4 py-2 text-xs font-medium bg-[#D4AF37]/15 text-[#D4AF37] border border-[#D4AF37]/30 rounded-lg hover:bg-[#D4AF37]/25 transition-colors"
        >
          {t('playbook.sovereign.generateStackDoc')}
        </button>
        {profileCompleteness && profileCompleteness.missing.length > 0 && (
          <span className="text-[10px] text-[#666]">
            {t('playbook.sovereign.missing', { fields: profileCompleteness.missing.join(', ') })}
          </span>
        )}
      </div>

      {/* Generated Document Display */}
      {showDocument && generatedDocument && (
        <div className="border border-border rounded-lg bg-bg-primary p-4 mt-2">
          <div className="flex items-center justify-between mb-3">
            <span className="text-xs font-medium text-white">{t('playbook.sovereign.stackDocument')}</span>
            <div className="flex items-center gap-2">
              <button
                onClick={() => navigator.clipboard.writeText(generatedDocument)}
                className="text-[10px] text-[#666] hover:text-text-secondary transition-colors"
              >
                {t('action.copy')}
              </button>
              <button
                onClick={() => setShowDocument(false)}
                className="text-[10px] text-[#666] hover:text-text-secondary transition-colors"
              >
                {t('action.close')}
              </button>
            </div>
          </div>
          <pre className="text-xs text-text-secondary font-mono whitespace-pre-wrap max-h-[400px] overflow-y-auto leading-relaxed">
            {generatedDocument}
          </pre>
        </div>
      )}
    </div>
  );
}
