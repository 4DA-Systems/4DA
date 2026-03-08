import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  affinity_score: number;
}

interface AntiTopic {
  topic: string;
  rejection_count: number;
  confidence: number;
  auto_detected: boolean;
}

interface LearningIndicatorProps {
  learnedAffinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  lastLearnedTopic?: { topic: string; direction: 'positive' | 'negative'; timestamp: number } | null;
}

export function LearningIndicator({
  learnedAffinities,
  antiTopics,
  lastLearnedTopic,
}: LearningIndicatorProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [glowingTopic, setGlowingTopic] = useState<string | null>(null);
  const prevLearnedRef = useRef<typeof lastLearnedTopic>(null);

  // Total count for collapsed display
  const totalPreferences = learnedAffinities.length + antiTopics.length;

  // When lastLearnedTopic changes, trigger glow effect on the relevant pill
  useEffect(() => {
    if (
      lastLearnedTopic &&
      lastLearnedTopic.timestamp &&
      Date.now() - lastLearnedTopic.timestamp < 3000
    ) {
      // Only trigger if it's actually a new event
      const prev = prevLearnedRef.current;
      if (
        !prev ||
        prev.topic !== lastLearnedTopic.topic ||
        prev.timestamp !== lastLearnedTopic.timestamp
      ) {
        setGlowingTopic(lastLearnedTopic.topic.toLowerCase());
        const timer = setTimeout(() => setGlowingTopic(null), 2000);
        return () => clearTimeout(timer);
      }
    }
    prevLearnedRef.current = lastLearnedTopic;
  }, [lastLearnedTopic]);

  // Don't render if nothing learned yet
  if (totalPreferences === 0) {
    return null;
  }

  // Top 5 positive affinities (positive score, sorted by score descending)
  const positiveAffinities = learnedAffinities
    .filter((a) => a.affinity_score > 0)
    .sort((a, b) => b.affinity_score - a.affinity_score)
    .slice(0, 5);

  // Top anti-topics (fill remaining slots up to 5 total pills)
  const remainingSlots = Math.max(0, 5 - positiveAffinities.length);
  const displayAntiTopics = antiTopics
    .sort((a, b) => b.rejection_count - a.rejection_count)
    .slice(0, Math.max(2, remainingSlots));

  return (
    <div
      className="mb-6 bg-bg-secondary border border-border rounded-lg transition-all duration-300 ease-in-out"
    >
      {/* Collapsed / Header Row */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-4 py-2 flex items-center gap-3 hover:bg-[#1A1A1A] transition-colors cursor-pointer"
        aria-expanded={expanded}
        aria-label={expanded ? 'Collapse learning preferences' : 'Expand learning preferences'}
      >
        {/* Brain icon */}
        <div className="w-6 h-6 bg-bg-tertiary rounded flex items-center justify-center flex-shrink-0">
          <svg
            className="w-3.5 h-3.5 text-orange-400"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 2a7 7 0 0 0-7 7c0 2.38 1.19 4.47 3 5.74V17a2 2 0 0 0 2 2h4a2 2 0 0 0 2-2v-2.26c1.81-1.27 3-3.36 3-5.74a7 7 0 0 0-7-7z" />
            <path d="M9 21h6" />
            <path d="M10 17v4" />
            <path d="M14 17v4" />
          </svg>
        </div>

        <span className="text-xs text-gray-400 flex-1 text-left">
          {t('learnedBehavior.learningCount', { count: totalPreferences })}
        </span>

        {/* Expand/Collapse chevron */}
        <svg
          className={`w-3.5 h-3.5 text-gray-500 transition-transform duration-200 ${expanded ? 'rotate-180' : ''}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {/* Expanded Content */}
      <div
        className={`transition-all duration-300 ease-in-out overflow-hidden ${
          expanded ? 'max-h-96 opacity-100' : 'max-h-0 opacity-0'
        }`}
      >
        <div className="px-4 pb-3 pt-1 flex flex-wrap gap-2">
          {/* Positive affinity pills */}
          {positiveAffinities.map((affinity) => {
            const isGlowing =
              glowingTopic === affinity.topic.toLowerCase();
            return (
              <span
                key={`pos-${affinity.topic}`}
                className={`inline-flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg border transition-all duration-500 ${
                  isGlowing
                    ? 'bg-green-500/20 text-green-300 border-green-400/50 shadow-[0_0_8px_rgba(34,197,94,0.3)]'
                    : 'bg-green-500/10 text-green-400 border-green-500/20'
                }`}
                title={`Affinity score: ${Math.round(affinity.affinity_score * 100)}% | +${affinity.positive_signals} / -${affinity.negative_signals}`}
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
                </svg>
                {affinity.topic} +{affinity.positive_signals}
              </span>
            );
          })}

          {/* Anti-topic pills */}
          {displayAntiTopics.map((anti) => {
            const isGlowing =
              glowingTopic === anti.topic.toLowerCase();
            return (
              <span
                key={`anti-${anti.topic}`}
                className={`inline-flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg border transition-all duration-500 ${
                  isGlowing
                    ? 'bg-red-500/20 text-red-300 border-red-400/50 shadow-[0_0_8px_rgba(239,68,68,0.3)]'
                    : 'bg-red-500/10 text-red-400 border-red-500/20'
                }`}
                title={`Rejected ${anti.rejection_count} times | Confidence: ${Math.round(anti.confidence * 100)}%${anti.auto_detected ? ' (auto-detected)' : ''}`}
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth={2.5}>
                  <path strokeLinecap="round" strokeLinejoin="round" d="M20 12H4" />
                </svg>
                {anti.topic} -{anti.rejection_count}
              </span>
            );
          })}

          {/* Overflow indicator */}
          {learnedAffinities.filter((a) => a.affinity_score > 0).length > 5 && (
            <span className="text-[11px] text-gray-500 self-center">
              +{learnedAffinities.filter((a) => a.affinity_score > 0).length - 5} more
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
