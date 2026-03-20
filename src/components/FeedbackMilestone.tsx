import { memo, useState, useEffect } from 'react';

const MILESTONES: Record<number, string> = {
  10: 'Your 4DA learned from 10 signals this session',
  50: '50 signals — your model is now personalized',
  100: '100 signals — top 5% of active users',
  500: '500 signals — deeply calibrated',
};

interface FeedbackMilestoneProps {
  count: number;
}

export const FeedbackMilestone = memo(function FeedbackMilestone({ count }: FeedbackMilestoneProps) {
  const [visible, setVisible] = useState(false);
  const message = MILESTONES[count];

  useEffect(() => {
    if (!message) return;
    setVisible(true);
    const timer = setTimeout(() => setVisible(false), 5000);
    return () => clearTimeout(timer);
  }, [message]);

  if (!message || !visible) return null;

  return (
    <div
      role="status"
      className="fixed bottom-6 right-6 z-50 flex items-center gap-3 px-5 py-3.5
        bg-[#141414] border border-[#22C55E]/30 rounded-xl shadow-lg shadow-[#22C55E]/5
        animate-[slideUp_0.3s_ease-out]"
    >
      <div className="flex items-center justify-center w-8 h-8 rounded-full bg-[#22C55E]/15">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M4 8.5L7 11.5L12 5" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </div>
      <div>
        <p className="text-sm font-medium text-white">{message}</p>
        <p className="text-[11px] text-[#A0A0A0] mt-0.5">Your feed keeps getting sharper</p>
      </div>
    </div>
  );
});
