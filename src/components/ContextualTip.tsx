import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';

const STORAGE_KEY = '4da-dismissed-tips';

function getDismissedTips(): Set<string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return new Set(JSON.parse(raw));
  } catch { /* ignore */ }
  return new Set();
}

function dismissTip(tipId: string) {
  try {
    const dismissed = getDismissedTips();
    dismissed.add(tipId);
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...dismissed]));
  } catch { /* ignore */ }
}

interface ContextualTipProps {
  tipId: string;
  message: string;
  hint?: string;
  /** Only show after this condition is true */
  showWhen?: boolean;
}

/** Small, dismissable contextual tip. Tracks dismissal in localStorage. */
export const ContextualTip = memo(function ContextualTip({ tipId, message, hint, showWhen = true }: ContextualTipProps) {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (!showWhen) return;
    const dismissed = getDismissedTips();
    if (!dismissed.has(tipId)) {
      setVisible(true);
    }
  }, [tipId, showWhen]);

  if (!visible) return null;

  return (
    <div className="mb-3 px-3 py-2.5 bg-amber-500/5 border border-amber-500/20 rounded-lg flex items-start gap-2">
      <span className="text-amber-400 text-xs mt-0.5 flex-shrink-0">{'\u{1F4A1}'}</span>
      <div className="flex-1 min-w-0">
        <p className="text-xs text-amber-400/90">{message}</p>
        {hint && <p className="text-[10px] text-amber-400/50 mt-0.5">{hint}</p>}
      </div>
      <button
        onClick={() => { setVisible(false); dismissTip(tipId); }}
        className="text-[10px] text-text-muted hover:text-text-secondary flex-shrink-0"
        aria-label={t('action.dismiss', 'Dismiss')}
      >
        {'\u2715'}
      </button>
    </div>
  );
});
