import { useState, useEffect, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ProGate } from './ProGate';
import type { PredictedContext } from '../types';

function confidenceColor(score: number): string {
  if (score >= 0.7) return '#2DD4BF'; // teal-400
  if (score >= 0.4) return '#5EEAD4'; // teal-300 faded
  return '#6B7280'; // gray-500
}

export const PredictiveIndicator = memo(function PredictiveIndicator() {
  const [prediction, setPrediction] = useState<PredictedContext | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const p = await invoke<PredictedContext>('get_predicted_context');
        if (!cancelled && p.predicted_topics.length > 0) setPrediction(p);
      } catch { /* prediction is optional */ }
    };
    load();
    const interval = setInterval(load, 5 * 60 * 1000);
    return () => { cancelled = true; clearInterval(interval); };
  }, []);

  if (!prediction || prediction.predicted_topics.length === 0) return null;

  const topTopics = prediction.predicted_topics.slice(0, 3);
  const maxScore = Math.max(...prediction.predicted_topics.map(([, s]) => s), 0.01);
  const predTime = new Date(prediction.predicted_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  return (
    <ProGate feature="Predictive Context">
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        className="w-full px-5 py-4 flex items-center justify-between hover:bg-[#1A1A1A] transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-teal-400">
              <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="1.5" fill="none" />
              <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeWidth="1" strokeDasharray="3 2" fill="none" opacity="0.5" />
            </svg>
          </div>
          <div className="text-left">
            <h2 className="font-medium text-white text-sm">Predicted Context</h2>
            <p className="text-xs text-gray-500">
              {topTopics.map(([t]) => t).join(', ')}
              <span className="ml-1.5 text-teal-400/70">{Math.round(prediction.confidence * 100)}%</span>
            </p>
          </div>
        </div>
        <span className="text-gray-500 text-sm">{expanded ? '\u25BE' : '\u25B8'}</span>
      </button>

      {expanded && (
        <div className="px-5 pb-4 pt-1 border-t border-border space-y-4">
          {/* Topic distribution bars */}
          <div className="space-y-2.5">
            {prediction.predicted_topics.map(([topic, score]) => (
              <div key={topic} className="flex items-center gap-3">
                <span className="text-xs text-gray-300 w-28 truncate shrink-0">{topic}</span>
                <div className="flex-1 h-2 bg-bg-primary rounded-full overflow-hidden">
                  <div
                    className="h-full rounded-full transition-all duration-500"
                    style={{
                      width: `${Math.max((score / maxScore) * 100, 4)}%`,
                      backgroundColor: confidenceColor(score),
                    }}
                  />
                </div>
                <span className="text-[11px] text-gray-500 w-10 text-right tabular-nums">
                  {Math.round(score * 100)}%
                </span>
              </div>
            ))}
          </div>

          {/* Reasoning */}
          {prediction.reasoning && (
            <p className="text-xs text-gray-500 leading-relaxed">{prediction.reasoning}</p>
          )}

          {/* Footer: prediction time + refresh cycle */}
          <div className="flex items-center justify-between pt-1">
            <span className="text-[10px] text-gray-600">Predicted at {predTime}</span>
            <span className="text-[10px] text-gray-600 flex items-center gap-1">
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" className="text-gray-600">
                <path d="M5 1v4l2.5 1.5" stroke="currentColor" strokeWidth="1" strokeLinecap="round" />
                <circle cx="5" cy="5" r="4" stroke="currentColor" strokeWidth="1" fill="none" />
              </svg>
              Refreshes every 5m
            </span>
          </div>
        </div>
      )}
    </div>
    </ProGate>
  );
});
