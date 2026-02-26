
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

interface LearnedBehaviorPanelProps {
  affinities: TopicAffinity[];
  antiTopics: AntiTopic[];
  onRefresh: () => void;
}

export function LearnedBehaviorPanel({
  affinities,
  antiTopics,
  onRefresh,
}: LearnedBehaviorPanelProps) {
  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-indigo-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-indigo-400">🧠</span>
        </div>
        <div>
          <h3 className="text-white font-medium">Learned Behavior</h3>
          <p className="text-gray-500 text-sm mt-1">
            Topics the system has learned from your feedback
          </p>
        </div>
      </div>

      {/* Affinities */}
      {affinities.length > 0 ? (
        <div className="space-y-3">
          <div className="text-xs text-gray-400 font-medium">
            Topic Affinities
          </div>
          <div className="space-y-1.5 max-h-40 overflow-y-auto">
            {affinities.slice(0, 10).map((affinity, i) => (
              <div
                key={i}
                className="flex items-center gap-2 text-xs bg-bg-secondary rounded-lg px-3 py-2 border border-border group hover:border-indigo-500/30 transition-colors"
              >
                <div
                  className={`w-14 text-center font-mono font-medium ${
                    affinity.affinity_score > 0.3
                      ? 'text-green-400'
                      : affinity.affinity_score < -0.3
                      ? 'text-red-400'
                      : 'text-gray-500'
                  }`}
                >
                  {affinity.affinity_score > 0 ? '+' : ''}
                  {(affinity.affinity_score * 100).toFixed(0)}%
                </div>
                <div className="flex-1 text-white truncate">
                  {affinity.topic}
                </div>
                <div className="text-gray-500 text-[10px]">
                  <span className="text-green-400/70">+{affinity.positive_signals}</span>
                  {' / '}
                  <span className="text-red-400/70">-{affinity.negative_signals}</span>
                </div>
              </div>
            ))}
          </div>
          {affinities.length > 10 && (
            <div className="text-xs text-gray-500 text-center">
              +{affinities.length - 10} more topics
            </div>
          )}
        </div>
      ) : (
        <div className="text-sm text-gray-500 bg-bg-secondary rounded-lg p-4 text-center border border-border">
          <div className="text-2xl mb-2">📊</div>
          <div>No learned affinities yet</div>
          <div className="text-xs text-gray-600 mt-1">
            Interact with results to train the system
          </div>
        </div>
      )}

      {/* Anti-Topics */}
      {antiTopics.length > 0 && (
        <div className="mt-4 space-y-3">
          <div className="text-xs text-gray-400 font-medium">
            Auto-Detected Anti-Topics
          </div>
          <div className="flex flex-wrap gap-1.5">
            {antiTopics.map((anti, i) => (
              <span
                key={i}
                className="text-xs bg-red-500/10 text-red-400 px-2.5 py-1 rounded-md border border-red-500/20"
                title={`Rejected ${anti.rejection_count}x, ${(
                  anti.confidence * 100
                ).toFixed(0)}% confidence`}
              >
                {anti.topic}
              </span>
            ))}
          </div>
          <div className="text-xs text-gray-500">
            Items with these topics will be penalized in relevance scoring
          </div>
        </div>
      )}

      {/* Refresh button */}
      <button
        onClick={onRefresh}
        className="mt-4 w-full px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-gray-400 hover:text-white hover:border-indigo-500/30 transition-all"
      >
        Refresh Learned Behavior
      </button>
    </div>
  );
}
