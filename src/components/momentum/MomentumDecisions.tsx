import { useMemo, useState, useCallback, memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { deriveDecisionPrompts } from './momentum-utils';

// ---------------------------------------------------------------------------
// Dismissed prompts (localStorage with 7-day TTL)
// ---------------------------------------------------------------------------

const DISMISSED_KEY = '4da_momentum_dismissed';

function getDismissed(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISSED_KEY);
    if (raw === null || raw === '') return new Set();
    const data = JSON.parse(raw) as Record<string, number>;
    const now = Date.now();
    const valid: Record<string, number> = {};
    for (const [id, ts] of Object.entries(data)) {
      if (now - ts < 7 * 24 * 60 * 60 * 1000) valid[id] = ts;
    }
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(valid));
    return new Set(Object.keys(valid));
  } catch { return new Set(); }
}

function dismissPrompt(id: string) {
  try {
    const raw = localStorage.getItem(DISMISSED_KEY);
    const data = (raw !== null && raw !== '') ? JSON.parse(raw) as Record<string, number> : {};
    data[id] = Date.now();
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(data));
  } catch { /* noop */ }
}

// ---------------------------------------------------------------------------
// Type config
// ---------------------------------------------------------------------------

const TYPE_STYLE: Record<string, { accent: string; bg: string }> = {
  track:     { accent: 'text-accent-gold', bg: 'bg-amber-500/10' },
  security:  { accent: 'text-red-400',     bg: 'bg-red-500/10' },
  declining: { accent: 'text-text-muted',  bg: 'bg-bg-tertiary' },
  version:   { accent: 'text-blue-400',    bg: 'bg-blue-500/10' },
  // Decision window types
  security_patch: { accent: 'text-red-400',    bg: 'bg-red-500/10' },
  migration:      { accent: 'text-amber-400',  bg: 'bg-amber-500/10' },
  adoption:       { accent: 'text-blue-400',   bg: 'bg-blue-500/10' },
  knowledge:      { accent: 'text-purple-400', bg: 'bg-purple-500/10' },
};

// ---------------------------------------------------------------------------
// Unified prompt (radar-derived + decision windows)
// ---------------------------------------------------------------------------

interface UnifiedPrompt {
  id: string;
  text: string;
  subtext: string;
  type: string;
  windowId?: number;  // present if from a decision window
}

// ---------------------------------------------------------------------------
// Prompt Card
// ---------------------------------------------------------------------------

const PromptCard = memo(function PromptCard({
  prompt,
  onYes,
  onNo,
  onLater,
  index,
}: {
  prompt: UnifiedPrompt;
  onYes: () => void;
  onNo: () => void;
  onLater: () => void;
  index: number;
}) {
  const { t } = useTranslation();
  const style = TYPE_STYLE[prompt.type] ?? TYPE_STYLE.track!;

  return (
    <div
      className="bg-bg-secondary rounded-lg border border-border p-3.5 flex items-start gap-3"
      style={{ animation: `slideInRight 0.4s ease-out ${index * 70}ms both` }}
    >
      <div className="flex-1 min-w-0">
        <p className="text-sm text-white font-medium leading-snug">{prompt.text}</p>
        <p className="text-[11px] text-text-muted mt-0.5">{prompt.subtext}</p>
      </div>
      <div className="flex items-center gap-1.5 flex-shrink-0">
        <button
          onClick={onYes}
          className={`px-2.5 py-1 text-[11px] font-medium rounded-md transition-colors ${style.accent} ${style.bg} hover:opacity-80`}
        >
          {t('momentum.yes')}
        </button>
        <button
          onClick={onNo}
          className="px-2 py-1 text-[11px] text-text-muted bg-bg-tertiary rounded-md hover:text-text-secondary transition-colors"
        >
          {t('momentum.no')}
        </button>
        <button
          onClick={onLater}
          className="px-2 py-1 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
          title={t('momentum.later')}
        >
          {t('momentum.later')}
        </button>
      </div>
    </div>
  );
});

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export interface MomentumDecisionsProps {
  entries: RadarEntry[];
  userStack: string[];
}

export const MomentumDecisions = memo(function MomentumDecisions({
  entries,
  userStack,
}: MomentumDecisionsProps) {
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState<Set<string>>(() => getDismissed());

  // Decision windows from store
  const windows = useAppStore(s => s.decisionWindows);
  const loadWindows = useAppStore(s => s.loadDecisionWindows);
  const actOnWindow = useAppStore(s => s.actOnWindow);
  const closeWindow = useAppStore(s => s.closeWindow);

  useEffect(() => { void loadWindows(); }, [loadWindows]);

  const prompts = useMemo(() => {
    const unified: UnifiedPrompt[] = [];

    // From decision windows
    const openWindows = (windows ?? []).filter(w => w.status === 'open');
    for (const w of openWindows) {
      unified.push({
        id: `window-${w.id}`,
        text: w.title,
        subtext: w.description ?? `${w.window_type} — urgency ${Math.round(w.urgency * 100)}%`,
        type: w.window_type,
        windowId: w.id,
      });
    }

    // From radar analysis
    const derived = deriveDecisionPrompts(entries, userStack);
    for (const d of derived) {
      unified.push({ id: d.id, text: d.text, subtext: d.subtext, type: d.type });
    }

    return unified.filter(p => !dismissed.has(p.id)).slice(0, 6);
  }, [entries, userStack, windows, dismissed]);

  const handleYes = useCallback((prompt: UnifiedPrompt) => () => {
    if (prompt.windowId !== undefined) void actOnWindow(prompt.windowId);
    dismissPrompt(prompt.id);
    setDismissed(prev => new Set([...prev, prompt.id]));
  }, [actOnWindow]);

  const handleNo = useCallback((prompt: UnifiedPrompt) => () => {
    if (prompt.windowId !== undefined) void closeWindow(prompt.windowId);
    dismissPrompt(prompt.id);
    setDismissed(prev => new Set([...prev, prompt.id]));
  }, [closeWindow]);

  const handleLater = useCallback((prompt: UnifiedPrompt) => () => {
    dismissPrompt(prompt.id);
    setDismissed(prev => new Set([...prev, prompt.id]));
  }, []);

  if (prompts.length === 0) return null;

  return (
    <div className="px-4 py-4">
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.decisionMoments')}
      </h3>
      <div className="space-y-2">
        {prompts.map((prompt, i) => (
          <PromptCard
            key={prompt.id}
            prompt={prompt}
            onYes={handleYes(prompt)}
            onNo={handleNo(prompt)}
            onLater={handleLater(prompt)}
            index={i}
          />
        ))}
      </div>
    </div>
  );
});
