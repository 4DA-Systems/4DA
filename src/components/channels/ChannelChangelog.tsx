import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ChannelChangelog as ChangelogType } from '../../types/channels';

interface Props {
  changelog: ChangelogType;
}

export function ChannelChangelog({ changelog }: Props) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const totalChanges =
    changelog.added_lines.length + changelog.removed_lines.length;
  if (totalChanges === 0) return null;

  return (
    <div className="mt-4 border border-border rounded-lg overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-4 py-2.5 bg-bg-secondary hover:bg-bg-tertiary transition-colors"
        aria-expanded={expanded}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-text-secondary">
            {t('channels.changelog')}
          </span>
          <span className="text-xs text-text-muted">{changelog.summary}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-text-muted">
            v{changelog.from_version} → v{changelog.to_version}
          </span>
          <svg
            className={`w-4 h-4 text-text-muted transition-transform ${
              expanded ? 'rotate-180' : ''
            }`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </div>
      </button>

      {expanded && (
        <div className="p-4 space-y-2 bg-bg-primary">
          {changelog.added_lines.map((line, i) => (
            <div
              key={`add-${i}`}
              className="px-3 py-2 text-xs text-green-400 bg-green-500/10 border-l-2 border-green-500 rounded-r font-mono"
            >
              + {line}
            </div>
          ))}
          {changelog.removed_lines.map((line, i) => (
            <div
              key={`rem-${i}`}
              className="px-3 py-2 text-xs text-red-400 bg-red-500/10 border-l-2 border-red-500 rounded-r font-mono"
            >
              - {line}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
