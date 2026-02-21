import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SandboxScoreResult {
  score: number;
  relevant: boolean;
  breakdown: {
    keyword_score: number;
    interest_score: number;
    ace_boost: number;
    affinity_mult: number;
    domain_relevance: number;
    content_quality: number;
  };
  matched_interests: string[];
  explanation: string | null;
}

const SOURCE_TYPES = [
  { value: 'sandbox', label: 'Sandbox' },
  { value: 'hackernews', label: 'Hacker News' },
  { value: 'reddit', label: 'Reddit' },
  { value: 'rss', label: 'RSS' },
  { value: 'arxiv', label: 'arXiv' },
  { value: 'github', label: 'GitHub' },
];

function scoreColor(score: number): string {
  if (score > 0.7) return '#22C55E';
  if (score > 0.4) return '#D4AF37';
  return '#EF4444';
}

function BreakdownBar({ label, value }: { label: string; value: number }) {
  const pct = Math.min(Math.max(value * 100, 0), 100);
  return (
    <div className="flex items-center gap-3 py-1">
      <span className="text-xs text-[#A0A0A0] w-36 shrink-0">{label}</span>
      <div className="flex-1 h-1.5 bg-white/10 rounded-full overflow-hidden">
        <div
          className="h-full bg-white rounded-full transition-all duration-500"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs font-mono text-[#A0A0A0] w-12 text-right">
        {value.toFixed(2)}
      </span>
    </div>
  );
}

export default function ScoringSandbox() {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [sourceType, setSourceType] = useState('sandbox');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SandboxScoreResult | null>(null);

  const score = useCallback(async () => {
    if (!title.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<SandboxScoreResult>('toolkit_score_sandbox', {
        title: title.trim(),
        content: content.trim() || null,
        sourceType: sourceType,
      });
      setResult(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setResult(null);
    } finally {
      setLoading(false);
    }
  }, [title, content, sourceType]);

  const reset = useCallback(() => {
    setTitle('');
    setContent('');
    setSourceType('sandbox');
    setResult(null);
    setError(null);
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && title.trim()) {
        score();
      }
    },
    [score, title],
  );

  const scorePct = result ? Math.round(result.score * 100) : 0;
  const color = result ? scoreColor(result.score) : '#666';

  return (
    <div className="space-y-4">
      {/* Input form */}
      <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-4 space-y-3">
        <div>
          <label className="block text-xs text-[#A0A0A0] mb-1.5">Title *</label>
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Enter article title to score..."
            className="w-full px-3 py-2 text-sm bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 transition-colors"
          />
        </div>

        <div>
          <label className="block text-xs text-[#A0A0A0] mb-1.5">Content (optional)</label>
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Paste article content or summary..."
            rows={3}
            className="w-full px-3 py-2 text-sm bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg text-white placeholder-[#666] focus:outline-none focus:border-white/30 transition-colors resize-y min-h-[60px]"
          />
        </div>

        <div className="flex items-end gap-3">
          <div className="flex-1 max-w-[200px]">
            <label className="block text-xs text-[#A0A0A0] mb-1.5">Source Type</label>
            <select
              value={sourceType}
              onChange={(e) => setSourceType(e.target.value)}
              className="w-full px-3 py-2 text-sm bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg text-white focus:outline-none focus:border-white/30 transition-colors"
            >
              {SOURCE_TYPES.map((st) => (
                <option key={st.value} value={st.value}>
                  {st.label}
                </option>
              ))}
            </select>
          </div>

          <button
            onClick={score}
            disabled={loading || !title.trim()}
            className="flex items-center gap-2 px-4 py-2 text-sm font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? (
              <>
                <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
                Scoring...
              </>
            ) : (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="12" cy="12" r="10" />
                  <line x1="12" y1="8" x2="12" y2="16" />
                  <line x1="8" y1="12" x2="16" y2="12" />
                </svg>
                Score
              </>
            )}
          </button>

          {(result || title || content) && (
            <button
              onClick={reset}
              className="px-3 py-2 text-xs text-[#A0A0A0] bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all"
            >
              Clear
            </button>
          )}
        </div>
      </div>

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

      {/* Results */}
      {result && (
        <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5 space-y-5">
          {/* Score hero */}
          <div className="flex items-center gap-5">
            <div className="text-center">
              <div className="text-5xl font-bold font-mono" style={{ color }}>
                {scorePct}
              </div>
              <div className="text-xs text-[#666] mt-0.5">/ 100</div>
            </div>
            <div>
              <span
                className="inline-flex items-center px-2.5 py-1 text-xs font-medium rounded-full"
                style={{
                  backgroundColor: `${color}15`,
                  color,
                  border: `1px solid ${color}30`,
                }}
              >
                {result.relevant ? 'Relevant' : 'Not Relevant'}
              </span>
            </div>
          </div>

          {/* Breakdown bars */}
          <div>
            <h4 className="text-xs font-medium text-[#A0A0A0] uppercase tracking-wider mb-2">
              Score Breakdown
            </h4>
            <div className="space-y-0.5">
              <BreakdownBar label="Keyword Score" value={result.breakdown.keyword_score} />
              <BreakdownBar label="Interest Score" value={result.breakdown.interest_score} />
              <BreakdownBar label="ACE Boost" value={result.breakdown.ace_boost} />
              <BreakdownBar label="Affinity Multiplier" value={result.breakdown.affinity_mult} />
              <BreakdownBar label="Domain Relevance" value={result.breakdown.domain_relevance} />
              <BreakdownBar label="Content Quality" value={result.breakdown.content_quality} />
            </div>
          </div>

          {/* Matched interests */}
          {result.matched_interests.length > 0 && (
            <div>
              <h4 className="text-xs font-medium text-[#A0A0A0] uppercase tracking-wider mb-2">
                Matched Interests
              </h4>
              <div className="flex flex-wrap gap-1.5">
                {result.matched_interests.map((interest) => (
                  <span
                    key={interest}
                    className="px-2 py-0.5 text-xs text-[#A0A0A0] bg-[#1F1F1F] border border-[#2A2A2A] rounded-full"
                  >
                    {interest}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Explanation */}
          {result.explanation && (
            <p className="text-xs text-[#666] italic leading-relaxed">
              {result.explanation}
            </p>
          )}
        </div>
      )}

      {/* Empty state */}
      {!result && !loading && !error && (
        <div className="flex flex-col items-center justify-center py-14 text-center">
          <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="#666" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
            <path d="M2 12h20" />
          </svg>
          <p className="text-sm text-[#A0A0A0] mb-1">Paste a title to test your scoring engine</p>
          <p className="text-xs text-[#666]">
            See how PASIFA scores content against your interest profile
          </p>
        </div>
      )}
    </div>
  );
}
