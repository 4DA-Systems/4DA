import { memo, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

const MILESTONE_KEYS: Record<number, string> = {
  10: 'feedback.milestone10',
  50: 'feedback.milestone50',
  100: 'feedback.milestone100',
  500: 'feedback.milestone500',
};

interface FeedbackMilestoneProps {
  count: number;
}

export const FeedbackMilestone = memo(function FeedbackMilestone({ count }: FeedbackMilestoneProps) {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);
  const milestoneKey = MILESTONE_KEYS[count];
  const message = milestoneKey ? t(milestoneKey) : undefined;

  useEffect(() => {
    if (!milestoneKey) return;
    setVisible(true);
    const timer = setTimeout(() => setVisible(false), 5000);
    return () => clearTimeout(timer);
  }, [milestoneKey]);

  if (!message || !visible) return null;

  return (
    <div
      role="status"
      className="fixed bottom-6 right-6 z-50 flex items-center gap-3 px-5 py-3.5
        bg-bg-secondary border border-success/30 rounded-xl shadow-lg shadow-success/5
        animate-[slideUp_0.3s_ease-out]"
    >
      <div className="flex items-center justify-center w-8 h-8 rounded-full bg-success/15">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M4 8.5L7 11.5L12 5" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </div>
      <div>
        <p className="text-sm font-medium text-white">{message}</p>
        <p className="text-[11px] text-text-secondary mt-0.5">{t('feedback.gettingSharper')}</p>
      </div>
    </div>
  );
});
