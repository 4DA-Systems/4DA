// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';

interface DefaultSourceListProps {
  defaults: string[];
  disabled: string[];
  onToggle: (item: string, enabled: boolean) => void;
  label: string;
}

export function DefaultSourceList({
  defaults,
  disabled,
  onToggle,
  label,
}: DefaultSourceListProps) {
  const [expanded, setExpanded] = useState(false);
  const activeCount = defaults.length - disabled.length;

  if (defaults.length === 0) return null;

  return (
    <div className="mt-1.5">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
      >
        <span
          className="inline-block transition-transform"
          style={{ transform: expanded ? 'rotate(90deg)' : 'rotate(0deg)' }}
        >
          &#9656;
        </span>
        <span>{label}</span>
        <span className="text-text-muted">
          ({activeCount}/{defaults.length} active)
        </span>
      </button>

      {expanded && (
        <div className="mt-1.5 space-y-0.5 max-h-40 overflow-y-auto">
          {defaults.map((item) => {
            const isEnabled = !disabled.includes(item);
            return (
              <div
                key={item}
                className="flex items-center justify-between px-2.5 py-1 bg-bg-secondary/50 rounded border border-border/50 group"
              >
                <span className="font-mono text-[11px] text-text-muted truncate flex-1 mr-2">
                  {item}
                </span>
                <button
                  onClick={() => onToggle(item, !isEnabled)}
                  className={`relative w-8 h-4 rounded-full transition-colors flex-shrink-0 ${
                    isEnabled ? 'bg-green-500/40' : 'bg-gray-600'
                  }`}
                  aria-label={isEnabled ? `Disable ${item}` : `Enable ${item}`}
                >
                  <span
                    className={`absolute top-0.5 w-3 h-3 rounded-full bg-white transition-transform ${
                      isEnabled ? 'translate-x-4' : 'translate-x-0.5'
                    }`}
                  />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
