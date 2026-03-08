import { useEffect, useState, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { DecisionWindow } from '../types/autophagy';

const TYPE_ICONS: Record<string, string> = {
  security_patch: '\u{1F6E1}',
  migration: '\u{1F504}',
  adoption: '\u{2728}',
  knowledge: '\u{1F4DA}',
};

const STATUS_STYLES: Record<string, string> = {
  open: 'text-amber-400 bg-amber-400/10',
  acted: 'text-green-400 bg-green-400/10',
  expired: 'text-red-400 bg-red-400/10',
  closed: 'text-text-muted bg-gray-500/10',
};

export const DecisionJournal = memo(function DecisionJournal() {
  const [windows, setWindows] = useState<DecisionWindow[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const result = await invoke<DecisionWindow[]>('get_decision_windows');
        setWindows(result);
      } catch {
        // Decision windows are supplementary
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  if (loading || windows.length === 0) return null;

  const open = windows.filter(w => w.status === 'open');
  const recent = windows
    .filter(w => w.status !== 'open')
    .slice(0, 3);

  if (open.length === 0 && recent.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-4 py-3 border-b border-border/50">
        <h3 className="text-sm font-medium text-white">Decision Windows</h3>
        <p className="text-xs text-text-muted mt-0.5">
          Time-bounded opportunities requiring attention
        </p>
      </div>

      <div className="divide-y divide-border/30">
        {open.map(w => (
          <div key={w.id} className="px-4 py-3">
            <div className="flex items-start gap-2">
              <span className="text-sm flex-shrink-0">
                {TYPE_ICONS[w.window_type] || '\u{1F4CB}'}
              </span>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-medium text-white truncate">
                    {w.title}
                  </span>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full ${STATUS_STYLES[w.status]}`}>
                    {w.status}
                  </span>
                </div>
                <p className="text-[11px] text-text-muted mt-0.5 line-clamp-2">
                  {w.description}
                </p>
                {w.expires_at && (
                  <p className="text-[10px] text-amber-400/70 mt-1">
                    Expires {new Date(w.expires_at).toLocaleDateString()}
                  </p>
                )}
              </div>
              {w.urgency > 0.7 && (
                <span className="text-[10px] text-red-400 font-mono flex-shrink-0">
                  urgent
                </span>
              )}
            </div>
          </div>
        ))}

        {recent.map(w => (
          <div key={w.id} className="px-4 py-2 opacity-60">
            <div className="flex items-center gap-2">
              <span className="text-xs">
                {TYPE_ICONS[w.window_type] || '\u{1F4CB}'}
              </span>
              <span className="text-[11px] text-text-secondary truncate flex-1">
                {w.title}
              </span>
              <span className={`text-[10px] px-1.5 py-0.5 rounded-full ${STATUS_STYLES[w.status]}`}>
                {w.status}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
});
