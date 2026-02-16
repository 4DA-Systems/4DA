import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { PredictedContext } from '../types';

export function PredictiveIndicator() {
  const [prediction, setPrediction] = useState<PredictedContext | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const p = await invoke<PredictedContext>('get_predicted_context');
        if (!cancelled && p.predicted_topics.length > 0) {
          setPrediction(p);
        }
      } catch {
        // Silently ignore — prediction is optional
      }
    };
    load();
    // Refresh prediction every 5 minutes
    const interval = setInterval(load, 5 * 60 * 1000);
    return () => { cancelled = true; clearInterval(interval); };
  }, []);

  if (!prediction || prediction.predicted_topics.length === 0) return null;

  const topTopics = prediction.predicted_topics.slice(0, 3);

  return (
    <div className="mb-4">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 px-3 py-2 bg-bg-secondary border border-purple-500/20 rounded-lg hover:border-purple-500/40 transition-all text-left w-full"
      >
        <span className="text-purple-400 text-sm">🔮</span>
        <span className="text-xs text-purple-300">
          Predicted: {topTopics.map(([topic]) => topic).join(', ')}
        </span>
        <span className="text-[10px] text-gray-600 ml-auto">
          {Math.round(prediction.confidence * 100)}% conf
        </span>
      </button>
      {expanded && (
        <div className="mt-2 px-4 py-3 bg-bg-secondary border border-purple-500/20 rounded-lg">
          <p className="text-xs text-gray-400 mb-2">{prediction.reasoning}</p>
          <div className="flex flex-wrap gap-2">
            {prediction.predicted_topics.map(([topic, score]) => (
              <span
                key={topic}
                className="px-2 py-1 text-[10px] bg-purple-500/10 text-purple-300 rounded border border-purple-500/20"
              >
                {topic} ({Math.round(score * 100)}%)
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
