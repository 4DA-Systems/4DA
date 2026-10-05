// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Side panel listing every item of one theme — the drill-down for a tile
// whose titles did not all fit. Choosing an item opens its detail panel.
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import type { GraphNode } from '../../types/graph';
import { ItemRow } from './ThemeMapParts';
import { displayThemeLabel, type Theme } from './theme-map-model';

interface Props {
  theme: Theme;
  isNew: (node: GraphNode) => boolean;
  onOpenItem: (node: GraphNode) => void;
  onClose: () => void;
}

export default function ThemeDetailPanel({ theme, isNew, onOpenItem, onClose }: Props) {
  const { t } = useTranslation();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div
      role="complementary"
      aria-label={t('signals.graphThemeAria')}
      className="absolute inset-y-0 end-0 z-20 w-[320px] flex flex-col border-s overflow-hidden"
      style={{ backgroundColor: 'var(--color-bg-secondary)', borderColor: 'var(--color-border)' }}
    >
      <div className="flex items-center justify-between px-3 py-2 border-b shrink-0" style={{ borderColor: 'var(--color-border)' }}>
        <div className="flex items-baseline gap-1.5 min-w-0">
          <span className="text-[13px] font-semibold text-text-primary truncate">{displayThemeLabel(theme.label)}</span>
          <span className="text-[11px] text-text-muted shrink-0">{theme.items.length}</span>
        </div>
        <button
          onClick={onClose}
          aria-label={t('action.close')}
          className="text-[11px] text-text-muted hover:text-text-secondary transition-colors ms-2 shrink-0"
        >
          {t('action.close')}
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-3 py-2">
        {theme.items.map((n) => (
          <ItemRow key={n.id} node={n} isNew={isNew(n)} selected={false} onOpen={onOpenItem} />
        ))}
      </div>
    </div>
  );
}
