import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';

interface QuickResultsStepProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

interface ScanResult {
  title: string;
  score: number;
  source: string;
  sourceId: string;
}

type ScanPhase = 'scanning' | 'scoring' | 'done' | 'error';

export function QuickResultsStep({ isAnimating, onComplete, onBack }: QuickResultsStepProps) {
  const [phase, setPhase] = useState<ScanPhase>('scanning');
  const [message, setMessage] = useState('Deep scanning HN, arXiv, Reddit, GitHub, RSS, YouTube...');
  const [results, setResults] = useState<ScanResult[]>([]);
  const [errorMessage, setErrorMessage] = useState('');

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        // Phase 1: Fetch all sources
        setPhase('scanning');
        setMessage('Deep scanning HN, arXiv, Reddit, GitHub, RSS, YouTube...');

        await invoke('run_deep_initial_scan');
        if (cancelled) return;

        // Phase 2: Score using the unified pipeline
        setPhase('scoring');
        setMessage('Analyzing hundreds of items for relevance...');

        await invoke('run_cached_analysis');
        if (cancelled) return;

        // Poll for completion (unified pipeline runs async)
        const pollForResults = async (): Promise<Array<{
          id: number;
          title: string;
          url: string | null;
          top_score: number;
          source_type: string;
          relevant: boolean;
        }>> => {
          for (let i = 0; i < 60; i++) { // max 60s
            if (cancelled) return [];
            const status = await invoke<{
              running: boolean;
              completed: boolean;
              results: Array<{
                id: number;
                title: string;
                url: string | null;
                top_score: number;
                source_type: string;
                relevant: boolean;
              }> | null;
            }>('get_analysis_status');

            if (status.results && status.results.length > 0) {
              return status.results;
            }
            if (!status.running && status.completed) {
              return status.results || [];
            }
            await new Promise(r => setTimeout(r, 1000));
          }
          return [];
        };

        const items = await pollForResults();
        if (cancelled) return;

        // Extract top 5 results
        const topResults = items
          .filter(r => r.relevant || r.top_score >= 0.3)
          .sort((a, b) => b.top_score - a.top_score)
          .slice(0, 5)
          .map(r => ({
            title: r.title,
            score: Math.round(r.top_score * 100),
            source: getSourceLabel(r.source_type),
            sourceId: r.source_type,
          }));

        setResults(topResults);
        setPhase('done');
        setMessage(`Found ${items.filter(r => r.relevant || r.top_score >= 0.3).length} relevant items!`);
      } catch (e) {
        if (cancelled) return;
        setPhase('error');
        setErrorMessage(`${e}`);
      }
    })();

    return () => { cancelled = true; };
  }, []);

  const handleEnter = async () => {
    try {
      await invoke('mark_onboarding_complete');
    } catch {
      // Non-critical - continue anyway
    }
    onComplete();
  };

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">Your Results</h2>
      <p className="text-gray-400 mb-6 text-center">
        {phase === 'done'
          ? 'Here are your top picks from across the internet.'
          : 'Building your personalized intelligence feed...'}
      </p>

      <div className="bg-bg-secondary p-6 rounded-lg mb-6">
        {/* Scanning phase */}
        {phase === 'scanning' && (
          <div className="text-center py-8">
            <div className="w-20 h-20 mx-auto mb-4 relative">
              <div className="absolute inset-0 rounded-full border-4 border-orange-500/20" />
              <div
                className="absolute inset-0 rounded-full border-4 border-orange-500 border-t-transparent animate-spin"
                style={{ animationDuration: '1.5s' }}
              />
              <span className="absolute inset-0 flex items-center justify-center text-3xl">&#x1f52c;</span>
            </div>
            <h3 className="text-white font-medium mb-2">Scanning Sources</h3>
            <p className="text-sm text-gray-400">{message}</p>
            <div className="flex flex-wrap justify-center gap-2 mt-4">
              <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse">HN</span>
              <span className="px-2 py-1 bg-purple-500/20 text-purple-300 text-xs rounded animate-pulse">arXiv</span>
              <span className="px-2 py-1 bg-blue-500/20 text-blue-300 text-xs rounded animate-pulse">Reddit</span>
              <span className="px-2 py-1 bg-green-500/20 text-green-300 text-xs rounded animate-pulse">GitHub</span>
            </div>
            <p className="text-xs text-gray-500 mt-4">This may take 10-20 seconds</p>
          </div>
        )}

        {/* Scoring phase */}
        {phase === 'scoring' && (
          <div className="text-center py-8">
            <div className="w-20 h-20 mx-auto mb-4 relative">
              <div className="absolute inset-0 rounded-full border-4 border-cyan-500/20" />
              <div className="absolute inset-0 rounded-full border-4 border-cyan-500 border-t-transparent animate-spin" />
              <span className="absolute inset-0 flex items-center justify-center text-3xl">&#x1f916;</span>
            </div>
            <h3 className="text-white font-medium mb-2">Analyzing Relevance</h3>
            <p className="text-sm text-gray-400">{message}</p>
            <div className="w-48 h-1 bg-bg-tertiary rounded-full mx-auto mt-4 overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-cyan-500 to-orange-500 rounded-full animate-pulse"
                style={{ width: '70%' }}
              />
            </div>
          </div>
        )}

        {/* Done phase - results */}
        {phase === 'done' && (
          <div>
            <div className="flex items-center gap-2 mb-4">
              <span className="w-8 h-8 bg-green-500/20 rounded-full flex items-center justify-center text-green-400">
                &#x2713;
              </span>
              <span className="text-white font-medium">{message}</span>
            </div>

            {results.length > 0 ? (
              <div className="space-y-2">
                {results.map((result, i) => (
                  <div
                    key={i}
                    className="flex items-center gap-3 p-3 bg-bg-tertiary rounded-lg"
                  >
                    <span className={`px-2 py-0.5 text-xs rounded flex-shrink-0 ${
                      getSourceColorClass(result.sourceId)
                    }`}>
                      {result.source}
                    </span>
                    <span className="flex-1 text-sm text-gray-300 truncate">{result.title}</span>
                    <span className="text-xs text-green-400 font-mono flex-shrink-0">{result.score}%</span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-center py-4 bg-bg-tertiary rounded-lg">
                <p className="text-gray-400">No highly relevant items found yet.</p>
                <p className="text-sm text-gray-500 mt-1">
                  4DA will learn your preferences as you give feedback.
                </p>
              </div>
            )}
          </div>
        )}

        {/* Error phase */}
        {phase === 'error' && (
          <div className="text-center py-8">
            <div className="w-16 h-16 mx-auto mb-4 bg-red-500/20 rounded-full flex items-center justify-center">
              <span className="text-3xl">&#x26a0;</span>
            </div>
            <h3 className="text-red-300 font-medium mb-2">Scan encountered an issue</h3>
            <p className="text-sm text-gray-400">{errorMessage}</p>
            <p className="text-xs text-gray-500 mt-2">You can try again from the main app.</p>
          </div>
        )}
      </div>

      {/* CTA and navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          disabled={phase === 'scanning' || phase === 'scoring'}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors disabled:opacity-50"
        >
          &larr; Back
        </button>
        <button
          onClick={handleEnter}
          className="px-10 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-all font-medium hover:scale-105 active:scale-95"
        >
          {phase === 'done' || phase === 'error' ? 'Enter 4DA' : 'Enter 4DA'}
        </button>
      </div>

      {(phase === 'scanning' || phase === 'scoring') && (
        <p className="text-xs text-gray-500 text-center mt-3">
          Full scan continues in background - you can enter anytime.
        </p>
      )}
      {phase === 'done' && (
        <p className="text-xs text-gray-500 text-center mt-3">
          Full scan continues in background for comprehensive results.
        </p>
      )}
    </div>
  );
}
