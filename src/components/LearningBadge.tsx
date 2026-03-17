import { memo, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

export const LearningBadge = memo(function LearningBadge() {
  const { t } = useTranslation();
  const feedbackCount = useAppStore(s => Object.keys(s.feedbackGiven).length);
  const lastLearned = useAppStore(s => s.lastLearnedTopic);
  const [showTopic, setShowTopic] = useState(false);

  useEffect(() => {
    if (lastLearned && Date.now() - lastLearned.timestamp < 3000) {
      setShowTopic(true);
      const timer = setTimeout(() => setShowTopic(false), 2500);
      return () => clearTimeout(timer);
    }
  }, [lastLearned]);

  if (feedbackCount === 0) return null;

  return (
    <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-blue-500/10 border border-blue-500/20 rounded-lg transition-all">
      <div className="w-1.5 h-1.5 bg-blue-400 rounded-full animate-pulse" />
      <span className="text-[11px] text-blue-400 font-medium tabular-nums">
        {showTopic && lastLearned
          ? `${lastLearned.direction === 'positive' ? '+' : '-'} ${lastLearned.topic}`
          : t('header.signalsLearned', { count: feedbackCount, defaultValue: `${feedbackCount} learned` })}
      </span>
    </div>
  );
});
