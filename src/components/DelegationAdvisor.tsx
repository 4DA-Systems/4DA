import { useEffect, useState, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ProGate } from './ProGate';

interface DelegationScore {
  technology: string;
  overall_score: number;
  recommendation: string;
  factors: {
    complexity: number;
    risk: number;
    maturity: number;
    reversibility: number;
    security_sensitivity: number;
  };
  caveats: string[];
}

const REC_COLORS: Record<string, string> = {
  FullyDelegate: 'text-green-400',
  DelegateWithReview: 'text-blue-400',
  CollaborateRealtime: 'text-amber-400',
  HumanOnly: 'text-red-400',
};

const REC_LABELS: Record<string, string> = {
  FullyDelegate: 'Safe to Delegate',
  DelegateWithReview: 'Delegate with Review',
  CollaborateRealtime: 'Collaborate',
  HumanOnly: 'Human Only',
};

const REC_ORDER = ['HumanOnly', 'CollaborateRealtime', 'DelegateWithReview', 'FullyDelegate'];

function FactorBar({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-text-muted w-20 shrink-0">{label}</span>
      <div className="flex-1 h-1 bg-white/5 rounded-full overflow-hidden">
        <div className="h-full bg-white/30 rounded-full" style={{ width: `${Math.round(value * 100)}%` }} />
      </div>
      <span className="text-[10px] text-text-muted w-7 text-right">{(value * 100).toFixed(0)}%</span>
    </div>
  );
}

export const DelegationAdvisor = memo(function DelegationAdvisor() {
  const [scores, setScores] = useState<DelegationScore[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedTech, setExpandedTech] = useState<string | null>(null);

  useEffect(() => {
    invoke<DelegationScore[]>('get_all_delegation_scores')
      .then(setScores)
      .catch(() => setScores([]))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="bg-bg-secondary border border-border rounded-lg p-4">
        <div className="animate-pulse space-y-2">
          <div className="h-4 bg-white/5 rounded w-40" />
          <div className="h-3 bg-white/5 rounded w-64" />
        </div>
      </div>
    );
  }

  if (scores.length === 0) {
    return (
      <div className="bg-bg-secondary border border-border rounded-lg p-4">
        <h3 className="text-sm font-medium text-white mb-1">Delegation Advisor</h3>
        <p className="text-xs text-text-muted">No technologies assessed yet. Add projects and dependencies to see delegation recommendations.</p>
      </div>
    );
  }

  const grouped: Record<string, DelegationScore[]> = {};
  for (const s of scores) {
    if (!grouped[s.recommendation]) grouped[s.recommendation] = [];
    grouped[s.recommendation].push(s);
  }

  return (
    <ProGate feature="delegation_advisor">
      <div className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <h3 className="text-sm font-medium text-white">Delegation Advisor</h3>
        <p className="text-[10px] text-text-muted">What is safe to delegate to AI vs. what needs your review</p>
        {REC_ORDER.filter(r => grouped[r]).map(rec => (
          <div key={rec}>
            <h4 className={`text-[11px] font-medium mb-1.5 ${REC_COLORS[rec] || 'text-white'}`}>
              {REC_LABELS[rec] || rec} ({grouped[rec].length})
            </h4>
            <div className="space-y-1">
              {grouped[rec].map(s => {
                const isExpanded = expandedTech === s.technology;
                return (
                  <div key={s.technology} className="bg-bg-primary/50 rounded px-2 py-1.5">
                    <button className="w-full flex items-center justify-between text-left" onClick={() => setExpandedTech(isExpanded ? null : s.technology)}>
                      <span className="text-xs text-white">{s.technology}</span>
                      <span className="text-[10px] text-text-muted">{(s.overall_score * 100).toFixed(0)}%</span>
                    </button>
                    {isExpanded && (
                      <div className="mt-2 space-y-1.5">
                        <FactorBar label="Complexity" value={s.factors.complexity} />
                        <FactorBar label="Risk" value={s.factors.risk} />
                        <FactorBar label="Maturity" value={s.factors.maturity} />
                        <FactorBar label="Reversibility" value={s.factors.reversibility} />
                        <FactorBar label="Security" value={s.factors.security_sensitivity} />
                        {s.caveats.length > 0 && (
                          <div className="mt-1 pt-1 border-t border-border">
                            {s.caveats.map((c, i) => (
                              <p key={i} className="text-[10px] text-amber-400/70">{c}</p>
                            ))}
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
    </ProGate>
  );
});
