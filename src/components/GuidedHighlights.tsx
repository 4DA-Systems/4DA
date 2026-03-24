import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

const STORAGE_KEY = '4da_highlights_shown';
const AUTO_DISMISS_MS = 30000;
const TOOLTIP_DURATION_MS = 3000;

interface Highlight {
  id: string;
  i18nKey: string;
  top: string;
  left: string;
}

const highlights: Highlight[] = [
  { id: 'feed', i18nKey: 'guided.feed', top: '48px', left: '80px' },
  { id: 'score', i18nKey: 'guided.score', top: '140px', left: '64px' },
  { id: 'save', i18nKey: 'guided.save', top: '140px', left: 'calc(100% - 120px)' },
  { id: 'dna', i18nKey: 'guided.dna', top: '48px', left: '200px' },
  { id: 'settings', i18nKey: 'guided.settings', top: '48px', left: 'calc(100% - 48px)' },
];

export function GuidedHighlights() {
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const [activeTooltip, setActiveTooltip] = useState<string | null>(null);
  const [unmounted, setUnmounted] = useState(false);

  const [alreadyShown] = useState(() => {
    try { return localStorage.getItem(STORAGE_KEY) === 'true'; } catch { return false; }
  });

  const markComplete = useCallback(() => {
    try { localStorage.setItem(STORAGE_KEY, 'true'); } catch { /* noop */ }
    setUnmounted(true);
  }, []);

  // Auto-dismiss after 30 seconds
  useEffect(() => {
    if (alreadyShown || unmounted) return;
    const timer = setTimeout(markComplete, AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [alreadyShown, unmounted, markComplete]);

  // Check if all dismissed
  useEffect(() => {
    if (dismissed.size === highlights.length && !unmounted) {
      const timer = setTimeout(markComplete, TOOLTIP_DURATION_MS + 200);
      return () => clearTimeout(timer);
    }
  }, [dismissed, unmounted, markComplete]);

  if (alreadyShown || unmounted) return null;

  const handleClick = (id: string) => {
    setDismissed(prev => new Set(prev).add(id));
    setActiveTooltip(id);
    setTimeout(() => setActiveTooltip(prev => prev === id ? null : prev), TOOLTIP_DURATION_MS);
  };

  const visible = highlights.filter(h => !dismissed.has(h.id));

  return (
    <div className="fixed inset-0 z-40 pointer-events-none" aria-label="Feature highlights overlay">
      {visible.map(h => (
        <button
          key={h.id}
          aria-label={`Feature highlight: ${t(h.i18nKey)}`}
          onClick={() => handleClick(h.id)}
          className="absolute pointer-events-auto cursor-pointer group"
          style={{ top: h.top, left: h.left }}
        >
          <span className="block w-3 h-3 rounded-full bg-accent-gold animate-pulse shadow-[0_0_8px_rgba(212,175,55,0.5)]" />
        </button>
      ))}
      {activeTooltip && (() => {
        const h = highlights.find(hl => hl.id === activeTooltip);
        if (!h) return null;
        return (
          <div
            className="absolute z-50 px-3 py-2 rounded-lg bg-bg-secondary border border-border text-xs text-white shadow-lg max-w-[200px] animate-in fade-in"
            style={{ top: `calc(${h.top} + 20px)`, left: h.left }}
          >
            {t(h.i18nKey)}
          </div>
        );
      })()}
    </div>
  );
}
