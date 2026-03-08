import { useTranslation } from 'react-i18next';
import type { Anomaly, SystemHealth } from '../types';

interface SimilarTopicResult {
  topic: string;
  similarity: number;
}

interface SystemHealthPanelProps {
  health: SystemHealth | null;
  similarTopicQuery: string;
  onSimilarTopicQueryChange: (query: string) => void;
  similarTopicResults: SimilarTopicResult[];
  onRunAnomalyDetection: () => void;
  onResolveAnomaly: (anomalyId: number) => void;
  onFindSimilarTopics: () => void;
  onSaveWatcherState: () => void;
  onRefresh: () => void;
}

export function SystemHealthPanel({
  health,
  similarTopicQuery,
  onSimilarTopicQueryChange,
  similarTopicResults,
  onRunAnomalyDetection,
  onResolveAnomaly,
  onFindSimilarTopics,
  onSaveWatcherState,
  onRefresh,
}: SystemHealthPanelProps) {
  const { t } = useTranslation();
  if (!health) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-rose-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
            <span className="text-rose-400">💓</span>
          </div>
          <div>
            <h3 className="text-white font-medium">{t('systemHealth.title')}</h3>
            <p className="text-text-muted text-sm">{t('systemHealth.loading')}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-rose-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-rose-400">💓</span>
        </div>
        <div>
          <h3 className="text-white font-medium">{t('systemHealth.title')}</h3>
          <p className="text-text-muted text-sm">
            {t('systemHealth.subtitle')}
          </p>
        </div>
      </div>

      <div className="space-y-4">
        {/* Service Status */}
        <div className="grid grid-cols-2 gap-3">
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted mb-1">{t('systemHealth.embedding')}</div>
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${health.embeddingOperational ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className={`text-sm font-medium ${health.embeddingOperational ? 'text-green-400' : 'text-red-400'}`}>
                {health.embeddingOperational ? t('systemHealth.operational') : t('status.offline')}
              </span>
            </div>
          </div>
          <div className="p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-muted mb-1">{t('systemHealth.rateLimit')}</div>
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${
                health.rateLimitStatus
                  ? health.rateLimitStatus.is_limited
                    ? 'bg-red-500'
                    : 'bg-green-500'
                  : 'bg-gray-500'
              }`} />
              <span className={`text-sm font-medium ${
                health.rateLimitStatus
                  ? health.rateLimitStatus.is_limited
                    ? 'text-red-400'
                    : 'text-green-400'
                  : 'text-text-muted'
              }`}>
                {health.rateLimitStatus
                  ? health.rateLimitStatus.is_limited
                    ? t('systemHealth.limited')
                    : t('systemHealth.remaining', { count: health.rateLimitStatus.global_remaining })
                  : t('systemHealth.na')}
              </span>
            </div>
          </div>
        </div>

        {/* Accuracy Metrics */}
        {health.accuracyMetrics && (
          <div className="p-4 bg-bg-secondary rounded-lg border border-border">
            <div className="text-xs text-text-secondary mb-3 font-medium">{t('systemHealth.accuracyMetrics')}</div>
            <div className="grid grid-cols-3 gap-3">
              <div className="text-center">
                <div className="text-lg font-semibold text-white">
                  {(health.accuracyMetrics.precision * 100).toFixed(0)}%
                </div>
                <div className="text-xs text-text-muted">{t('systemHealth.precision')}</div>
              </div>
              <div className="text-center">
                <div className="text-lg font-semibold text-white">
                  {(health.accuracyMetrics.engagement_rate * 100).toFixed(0)}%
                </div>
                <div className="text-xs text-text-muted">{t('systemHealth.engagement')}</div>
              </div>
              <div className="text-center">
                <div className="text-lg font-semibold text-white">
                  {(health.accuracyMetrics.calibration_error * 100).toFixed(1)}%
                </div>
                <div className="text-xs text-text-muted">{t('systemHealth.calibration')}</div>
              </div>
            </div>
            {health.accuracyMetrics.precision === 0 &&
              health.accuracyMetrics.engagement_rate === 0 && (
                <div className="text-xs text-text-muted mt-3 text-center">
                  {t('systemHealth.metricsUpdate')}
                </div>
              )}
          </div>
        )}

        {/* Anomaly Detection */}
        <div>
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary font-medium">{t('systemHealth.anomalies')}</span>
              <span className={`px-2 py-0.5 text-xs rounded-md ${
                health.anomalyCount > 0
                  ? 'bg-red-500/20 text-red-400'
                  : 'bg-green-500/20 text-green-400'
              }`}>
                {health.anomalyCount}
              </span>
            </div>
            <button
              onClick={onRunAnomalyDetection}
              className="px-3 py-1.5 text-xs bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-rose-500/30 transition-all"
            >
              {t('systemHealth.scanNow')}
            </button>
          </div>
          {health.anomalies.length > 0 ? (
            <div className="space-y-2 max-h-32 overflow-y-auto">
              {health.anomalies.map((anomaly, i) => (
                <AnomalyItem
                  key={anomaly.id || i}
                  anomaly={anomaly}
                  onResolve={onResolveAnomaly}
                />
              ))}
            </div>
          ) : (
            <div className="text-sm text-green-400 bg-green-500/10 rounded-lg p-3 border border-green-500/20 text-center">
              {t('systemHealth.noAnomalies')}
            </div>
          )}
        </div>

        {/* Topic Similarity Search */}
        <div>
          <label className="text-xs text-text-secondary font-medium block mb-2">
            {t('systemHealth.findSimilarTopics')}
          </label>
          <div className="flex gap-2 mb-3">
            <input
              type="text"
              value={similarTopicQuery}
              onChange={(e) => onSimilarTopicQueryChange(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && onFindSimilarTopics()}
              placeholder={t('systemHealth.topicPlaceholder')}
              className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-rose-500/50 focus:outline-none transition-colors"
            />
            <button
              onClick={onFindSimilarTopics}
              className="px-4 py-2.5 text-sm bg-rose-500/20 border border-rose-500/30 text-rose-400 rounded-lg hover:bg-rose-500/30 transition-all"
            >
              {t('action.search')}
            </button>
          </div>
          {similarTopicResults.length > 0 && (
            <div className="space-y-1.5">
              {similarTopicResults.map((result, i) => (
                <div
                  key={i}
                  className="flex items-center justify-between p-2.5 bg-bg-secondary rounded-lg border border-border"
                >
                  <span className="text-sm text-white">{result.topic}</span>
                  <span className="text-sm text-orange-400 font-medium">
                    {(result.similarity * 100).toFixed(1)}%
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Watcher State */}
        <div className="flex gap-2 pt-2">
          <button
            onClick={onSaveWatcherState}
            className="flex-1 px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-rose-500/30 transition-all"
          >
            {t('systemHealth.saveWatcherState')}
          </button>
          <button
            onClick={onRefresh}
            className="px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-rose-500/30 transition-all"
          >
            {t('action.refresh')}
          </button>
        </div>
      </div>
    </div>
  );
}

interface AnomalyItemProps {
  anomaly: Anomaly;
  onResolve: (anomalyId: number) => void;
}

function AnomalyItem({ anomaly, onResolve }: AnomalyItemProps) {
  const { t } = useTranslation();
  const isHighSeverity = anomaly.severity === 'high' || anomaly.severity === 'critical';
  const isMediumSeverity = anomaly.severity === 'medium';

  return (
    <div className={`p-3 rounded-lg border ${
      isHighSeverity
        ? 'bg-red-500/10 border-red-500/30'
        : isMediumSeverity
        ? 'bg-yellow-500/10 border-yellow-500/30'
        : 'bg-bg-secondary border-border'
    }`}>
      <div className="flex items-start justify-between gap-2">
        <div>
          <span className={`text-sm font-medium ${
            isHighSeverity
              ? 'text-red-400'
              : isMediumSeverity
              ? 'text-yellow-400'
              : 'text-text-secondary'
          }`}>
            {anomaly.anomaly_type.replace(/_/g, ' ')}
          </span>
          {anomaly.topic && (
            <span className="text-xs text-text-muted ml-2">({anomaly.topic})</span>
          )}
        </div>
        {anomaly.id && (
          <button
            onClick={() => onResolve(anomaly.id!)}
            className="text-xs text-green-400 hover:text-green-300 transition-colors"
          >
            {t('systemHealth.resolve')}
          </button>
        )}
      </div>
      <div className="text-xs text-text-muted mt-1">{anomaly.description}</div>
    </div>
  );
}
