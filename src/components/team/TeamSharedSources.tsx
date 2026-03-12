import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const SOURCE_ICONS: Record<string, string> = {
  hackernews: 'HN',
  reddit: 'R',
  rss: 'RSS',
  github: 'GH',
  arxiv: 'Ax',
  devto: 'D',
  lobsters: 'L',
};

export function TeamSharedSources() {
  const { t } = useTranslation();
  const sharedSources = useAppStore(s => s.sharedSources);
  const sharedSourcesLoading = useAppStore(s => s.sharedSourcesLoading);
  const loadSharedSources = useAppStore(s => s.loadSharedSources);
  const shareSource = useAppStore(s => s.shareSource);
  const upvoteSource = useAppStore(s => s.upvoteSource);
  const removeSharedSource = useAppStore(s => s.removeSharedSource);

  const [showAddForm, setShowAddForm] = useState(false);
  const [newSourceType, setNewSourceType] = useState('hackernews');
  const [newRecommendation, setNewRecommendation] = useState('');

  useEffect(() => {
    loadSharedSources();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleShare = async () => {
    if (!newRecommendation.trim()) return;
    await shareSource(newSourceType, '{}', newRecommendation.trim());
    setNewRecommendation('');
    setShowAddForm(false);
  };

  if (sharedSourcesLoading && sharedSources.length === 0) {
    return (
      <div className="flex items-center justify-center py-6">
        <span className="text-xs text-text-muted">{t('action.loading', 'Loading...')}</span>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-medium text-text-secondary">
          {t('team.sources.title', 'Shared Sources')} ({sharedSources.length})
        </h4>
        <button
          onClick={() => setShowAddForm(!showAddForm)}
          className="text-[10px] text-[#22C55E] hover:text-[#22C55E]/80 transition-colors"
        >
          {showAddForm ? t('action.cancel', 'Cancel') : t('team.sources.recommend', '+ Recommend')}
        </button>
      </div>

      {/* Add Form */}
      {showAddForm && (
        <div className="bg-bg-tertiary rounded-lg p-3 space-y-2 border border-border/50">
          <select
            value={newSourceType}
            onChange={e => setNewSourceType(e.target.value)}
            className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
            aria-label="Source type"
          >
            {Object.keys(SOURCE_ICONS).map(type => (
              <option key={type} value={type}>{type}</option>
            ))}
          </select>
          <input
            type="text"
            value={newRecommendation}
            onChange={e => setNewRecommendation(e.target.value)}
            placeholder={t('team.sources.whyRecommend', 'Why is this source valuable?')}
            className="w-full px-2 py-1.5 text-xs bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
            onKeyDown={e => e.key === 'Enter' && handleShare()}
          />
          <button
            onClick={handleShare}
            disabled={!newRecommendation.trim()}
            className="text-[10px] px-3 py-1.5 bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors disabled:opacity-50"
          >
            {t('team.sources.share', 'Share with Team')}
          </button>
        </div>
      )}

      {/* Source List */}
      {sharedSources.length === 0 ? (
        <p className="text-xs text-text-muted text-center py-6">
          {t('team.sources.empty', 'No shared sources yet. Recommend sources your team should follow.')}
        </p>
      ) : (
        <div className="space-y-1.5">
          {sharedSources.map(source => (
            <div
              key={source.id}
              className="flex items-center justify-between px-3 py-2.5 bg-bg-primary rounded-lg border border-border/50"
            >
              <div className="flex items-center gap-2.5 min-w-0">
                {/* Source Badge */}
                <span className="text-[10px] font-mono font-bold text-[#D4AF37] bg-[#D4AF37]/10 px-1.5 py-0.5 rounded shrink-0">
                  {SOURCE_ICONS[source.source_type] || source.source_type.slice(0, 3).toUpperCase()}
                </span>

                <div className="min-w-0">
                  <span className="text-xs text-white">{source.source_type}</span>
                  <p className="text-[10px] text-text-muted truncate">{source.recommendation}</p>
                </div>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                {/* Upvote */}
                <button
                  onClick={() => upvoteSource(source.id)}
                  className="flex items-center gap-1 text-[10px] text-text-muted hover:text-[#D4AF37] transition-colors"
                  aria-label={`Upvote ${source.source_type}`}
                >
                  <span>&#9650;</span>
                  <span>{source.upvotes}</span>
                </button>

                {/* Remove */}
                <button
                  onClick={() => removeSharedSource(source.id)}
                  className="text-[10px] text-text-muted hover:text-[#EF4444] transition-colors"
                  aria-label={`Remove ${source.source_type}`}
                >
                  &#10005;
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
