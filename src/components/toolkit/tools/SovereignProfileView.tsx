import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

// --- Types ---

interface SovereignProfile {
  categories: Record<string, Array<{ key: string; value: string; source: string }>>;
}

interface ProfileCompleteness {
  completeness: number;
  total_facts: number;
  categories_filled: number;
  total_categories: number;
}

// --- Constants ---

const CATEGORY_ORDER = [
  'CPU', 'RAM', 'GPU', 'Storage', 'Network', 'OS', 'LLM', 'Legal', 'Budget',
];

const CATEGORY_ICONS: Record<string, string> = {
  CPU: 'M18 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2z',
  RAM: 'M6 19v-3M10 19v-3M14 19v-3M18 19v-3M2 15h20M4 15V5a2 2 0 012-2h12a2 2 0 012 2v10',
  GPU: 'M4 6h16v12H4zM8 6V4M16 6V4',
  Storage: 'M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z',
  Network: 'M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5',
  OS: 'M20 16V7a2 2 0 00-2-2H6a2 2 0 00-2 2v9m16 0H4m16 0l1.28 2.55a1 1 0 01-.9 1.45H3.62a1 1 0 01-.9-1.45L4 16',
  LLM: 'M12 2a4 4 0 014 4c0 1.1-.9 2-2 2h-4a2 2 0 01-2-2 4 4 0 014-4zM8 14s1.5 2 4 2 4-2 4-2',
  Legal: 'M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z',
  Budget: 'M12 1v22M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6',
};

// --- Component ---

export default function SovereignProfileView() {
  const [profile, setProfile] = useState<SovereignProfile | null>(null);
  const [completeness, setCompleteness] = useState<ProfileCompleteness | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [document, setDocument] = useState<string | null>(null);
  const [docLoading, setDocLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [profileResult, completenessResult] = await Promise.all([
        invoke<SovereignProfile>('get_sovereign_profile'),
        invoke<ProfileCompleteness>('get_sovereign_profile_completeness'),
      ]);
      setProfile(profileResult);
      setCompleteness(completenessResult);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const generateDocument = useCallback(async () => {
    setDocLoading(true);
    try {
      const result = await invoke<{ document: string }>('generate_sovereign_stack_document');
      setDocument(result.document);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDocLoading(false);
    }
  }, []);

  const copyDocument = useCallback(async () => {
    if (!document) return;
    try {
      await navigator.clipboard.writeText(document);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setError('Failed to copy to clipboard');
    }
  }, [document]);

  // Build completeness ring
  const pct = completeness ? Math.round(completeness.completeness * 100) : 0;
  const circumference = 2 * Math.PI * 34; // radius = 34
  const dashOffset = circumference - (circumference * pct) / 100;

  return (
    <div className="space-y-4">
      {/* Error */}
      {error && (
        <div className="flex items-center gap-3 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-sm text-[#EF4444] flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-[#EF4444]/60 hover:text-[#EF4444] text-xs">
            Dismiss
          </button>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="flex flex-col items-center justify-center py-16 text-[#666]">
          <div className="w-8 h-8 border-2 border-[#2A2A2A] border-t-white rounded-full animate-spin mb-4" />
          <p className="text-sm">Loading sovereign profile...</p>
        </div>
      )}

      {/* Main content */}
      {!loading && profile && completeness && (
        <>
          {/* Completeness header */}
          <div className="flex items-center gap-6 bg-[#141414] border border-[#2A2A2A] rounded-xl p-4">
            {/* SVG Ring */}
            <div className="relative shrink-0" style={{ width: 80, height: 80 }}>
              <svg width="80" height="80" viewBox="0 0 80 80">
                <circle cx="40" cy="40" r="34" fill="none" stroke="#1F1F1F" strokeWidth="5" />
                <circle
                  cx="40" cy="40" r="34" fill="none"
                  stroke={pct > 70 ? '#22C55E' : pct > 40 ? '#D4AF37' : '#EF4444'}
                  strokeWidth="5" strokeLinecap="round"
                  strokeDasharray={circumference}
                  strokeDashoffset={dashOffset}
                  transform="rotate(-90 40 40)"
                  className="transition-all duration-700"
                />
              </svg>
              <div className="absolute inset-0 flex items-center justify-center">
                <span className="text-lg font-semibold text-white font-mono">{pct}%</span>
              </div>
            </div>

            <div className="flex-1 min-w-0">
              <h3 className="text-sm font-medium text-white mb-1">Profile Completeness</h3>
              <div className="flex items-center gap-4 text-xs text-[#A0A0A0]">
                <span>{completeness.total_facts} fact{completeness.total_facts !== 1 ? 's' : ''} collected</span>
                <span>{completeness.categories_filled}/{completeness.total_categories} categories filled</span>
              </div>
            </div>

            {/* Generate document button */}
            <button
              onClick={generateDocument}
              disabled={docLoading}
              className="flex items-center gap-2 px-4 py-2 text-xs font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
            >
              {docLoading ? (
                <>
                  <div className="w-3 h-3 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
                  Generating...
                </>
              ) : (
                <>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                    <polyline points="14 2 14 8 20 8" />
                    <line x1="16" y1="13" x2="8" y2="13" />
                    <line x1="16" y1="17" x2="8" y2="17" />
                  </svg>
                  Generate Document
                </>
              )}
            </button>
          </div>

          {/* Category grid */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {CATEGORY_ORDER.map((cat) => {
              const facts = profile.categories[cat] ?? [];
              const isFilled = facts.length > 0;
              const iconPath = CATEGORY_ICONS[cat] ?? CATEGORY_ICONS['CPU'];

              return (
                <div
                  key={cat}
                  className={`bg-[#141414] border rounded-lg p-3 transition-colors ${
                    isFilled ? 'border-[#2A2A2A]' : 'border-[#2A2A2A]/50 opacity-50'
                  }`}
                >
                  <div className="flex items-center gap-2 mb-2">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke={isFilled ? '#D4AF37' : '#666'} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d={iconPath} />
                    </svg>
                    <span className={`text-xs font-medium uppercase tracking-wide ${isFilled ? 'text-white' : 'text-[#666]'}`}>
                      {cat}
                    </span>
                    <span className="text-[10px] text-[#666] font-mono ml-auto">
                      {facts.length} fact{facts.length !== 1 ? 's' : ''}
                    </span>
                  </div>

                  {isFilled ? (
                    <div className="space-y-1 max-h-28 overflow-y-auto">
                      {facts.map((fact, i) => (
                        <div key={i} className="flex items-baseline gap-1.5 text-xs">
                          <span className="text-[#A0A0A0] shrink-0">{fact.key}:</span>
                          <span className="text-white font-mono truncate" title={fact.value}>{fact.value}</span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="text-[10px] text-[#666] italic">
                      Run STREETS commands to populate
                    </p>
                  )}
                </div>
              );
            })}
          </div>

          {/* Generated document */}
          {document && (
            <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-xs font-medium text-[#A0A0A0] uppercase tracking-wider">
                  Stack Document
                </h3>
                <button
                  onClick={copyDocument}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-[#A0A0A0] bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all"
                >
                  {copied ? (
                    <>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                      <span className="text-[#22C55E]">Copied</span>
                    </>
                  ) : (
                    <>
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                        <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                      </svg>
                      Copy
                    </>
                  )}
                </button>
              </div>
              <pre className="text-xs font-mono text-[#A0A0A0] bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg p-3 overflow-auto max-h-64 whitespace-pre-wrap break-words">
                {document}
              </pre>
            </div>
          )}
        </>
      )}
    </div>
  );
}
