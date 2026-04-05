import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';

const STORAGE_KEY = '4da-weekly-summary-dismissed';
const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

interface WeeklySummaryData {
  totalCycles: number;
  itemsAnalyzed: number;
  calibrationAccuracy: number;
  accuracyTrend: number | null;
  newTopics: string[];
  suggestions: Array<{ topic: string; reason: string }>;
}

/** Shows once per week at top of briefing. Summarizes learning progress. */
export const WeeklyIntelligenceSummary = memo(function WeeklyIntelligenceSummary() {
  const { t } = useTranslation();
  const addToast = useAppStore(s => s.addToast);
  const [data, setData] = useState<WeeklySummaryData | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    // Check if we should show (once per 7 days)
    try {
      const lastDismissed = localStorage.getItem(STORAGE_KEY);
      if (lastDismissed && Date.now() - parseInt(lastDismissed, 10) < WEEK_MS) return;
    } catch { /* show it */ }

    const load = async () => {
      try {
        const [pulseResult, affinityResult, suggestionsResult] = await Promise.allSettled([
          cmd('get_intelligence_pulse'),
          cmd('ace_get_topic_affinities'),
          cmd('ace_get_suggested_interests'),
        ]);

        const pulse = pulseResult.status === 'fulfilled' ? pulseResult.value : null;
        const affinities = affinityResult.status === 'fulfilled' ? affinityResult.value : null;
        const suggestions = suggestionsResult.status === 'fulfilled' ? suggestionsResult.value : [];

        if (!pulse || pulse.total_cycles === 0) {
          setVisible(true);
          return;
        }

        // Find new topics (high positive affinity, recent interactions)
        const newTopics: string[] = [];
        if (affinities?.affinities) {
          for (const a of affinities.affinities as Array<{ topic: string; affinity_score: number; positive_signals: number }>) {
            if (a.affinity_score > 0.3 && a.positive_signals >= 3) {
              newTopics.push(a.topic);
            }
          }
        }

        // Find interest suggestions (topics with saves but not in interests)
        const suggestionsFormatted = (suggestions as Array<{ topic: string; source: string; already_declared: boolean }>)
          .filter(s => !s.already_declared)
          .slice(0, 3)
          .map(s => ({ topic: s.topic, reason: s.source }));

        setData({
          totalCycles: pulse.total_cycles,
          itemsAnalyzed: pulse.items_analyzed_7d || 0,
          calibrationAccuracy: Math.round((pulse.calibration_accuracy || 0) * 100),
          accuracyTrend: null,
          newTopics: newTopics.slice(0, 5),
          suggestions: suggestionsFormatted,
        });
        setVisible(true);
      } catch {
        // Non-critical — just don't show
      }
    };

    load();
  }, []);

  const dismiss = () => {
    setVisible(false);
    try { localStorage.setItem(STORAGE_KEY, String(Date.now())); } catch { /* ignore */ }
  };

  const handleShare = () => {
    if (!data) return;
    const lines = [
      'My 4DA Weekly Intelligence Summary',
      '',
      `${data.itemsAnalyzed} items analyzed`,
      `${data.totalCycles} learning cycles`,
      `${data.calibrationAccuracy}% accuracy`,
    ];
    if (data.newTopics.length > 0) {
      lines.push(`Top topics: ${data.newTopics.join(', ')}`);
    }
    lines.push('', 'All signal. No feed.', 'https://4da.ai');
    navigator.clipboard.writeText(lines.join('\n')).then(() => {
      addToast('success', t('report.weeklyShareCopied'));
    }).catch(() => {
      // Clipboard write failed silently
    });
  };

  const addInterest = async (topic: string) => {
    try {
      await cmd('add_interest', { topic });
      addToast('success', t('weekly.interestAdded', { topic, defaultValue: `Added '${topic}' to interests` }));
      setData(prev => prev ? { ...prev, suggestions: prev.suggestions.filter(s => s.topic !== topic) } : null);
    } catch {
      addToast('error', t('weekly.interestFailed', { defaultValue: 'Failed to add interest' }));
    }
  };

  if (!visible) return null;

  if (!data) return (
    <div className="bg-bg-secondary rounded-lg border border-border p-4">
      <p className="text-sm text-text-muted">{t('weekly.comingSoon')}</p>
    </div>
  );

  const trendArrow = data.accuracyTrend != null
    ? data.accuracyTrend > 2 ? '\u2191' : data.accuracyTrend < -2 ? '\u2193' : '\u2192'
    : null;
  const trendColor = data.accuracyTrend != null
    ? data.accuracyTrend > 2 ? 'text-green-400' : data.accuracyTrend < -2 ? 'text-red-400' : 'text-text-muted'
    : '';

  return (
    <div className="mb-4 bg-bg-secondary rounded-lg border border-blue-500/20 overflow-hidden">
      <div className="px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-6 h-6 bg-blue-500/10 rounded flex items-center justify-center">
            <span className="text-xs text-blue-400">{'\u{1F4CA}'}</span>
          </div>
          <span className="text-sm font-medium text-white">
            {t('weekly.title', 'Weekly Intelligence Summary')}
          </span>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleShare}
            className="text-xs text-blue-400 hover:text-blue-300 font-medium transition-colors"
          >
            {t('report.shareReport')}
          </button>
          <button
            onClick={dismiss}
            className="text-xs text-text-muted hover:text-text-secondary transition-colors"
          >
            {t('action.dismiss', 'Dismiss')}
          </button>
        </div>
      </div>

      <div className="px-4 pb-3 space-y-3">
        {/* Stats row */}
        <div className="flex gap-6">
          <div>
            <span className="text-lg font-bold text-white tabular-nums">{data.itemsAnalyzed}</span>
            <p className="text-[10px] text-text-muted">{t('weekly.analyzed', 'items analyzed')}</p>
          </div>
          <div>
            <span className="text-lg font-bold text-white tabular-nums">{data.totalCycles}</span>
            <p className="text-[10px] text-text-muted">{t('weekly.cycles', 'learning cycles')}</p>
          </div>
          <div>
            <span className="text-lg font-bold text-white tabular-nums">
              {data.calibrationAccuracy}%
              {trendArrow && <span className={`ms-1 text-sm ${trendColor}`}>{trendArrow}</span>}
            </span>
            <p className="text-[10px] text-text-muted">{t('weekly.accuracy', 'accuracy')}</p>
          </div>
        </div>

        {/* New topics detected */}
        {data.newTopics.length > 0 && (
          <div>
            <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
              {t('weekly.topTopics', 'Top learned topics')}
            </p>
            <div className="flex flex-wrap gap-1">
              {data.newTopics.map(topic => (
                <span key={topic} className="px-2 py-0.5 text-xs bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 rounded">
                  {topic}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Suggestions */}
        {data.suggestions.length > 0 && (
          <div>
            <p className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
              {t('weekly.suggestions', 'Suggested interests')}
            </p>
            {data.suggestions.map(s => (
              <div key={s.topic} className="flex items-center justify-between py-1">
                <div>
                  <span className="text-xs text-text-primary">{s.topic}</span>
                  <span className="text-[10px] text-text-muted ms-2">{s.reason}</span>
                </div>
                <button
                  onClick={() => addInterest(s.topic)}
                  className="text-[10px] text-blue-400 hover:text-blue-300 font-medium"
                >
                  {t('weekly.addInterest', '+ Add')}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
});
