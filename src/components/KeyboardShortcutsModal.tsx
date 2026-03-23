import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

interface KeyboardShortcutsModalProps {
  onClose: () => void;
}

export function KeyboardShortcutsModal({ onClose }: KeyboardShortcutsModalProps) {
  const { t } = useTranslation();
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
    return () => {
      const trigger = document.querySelector<HTMLElement>('[data-shortcut-trigger]');
      trigger?.focus();
    };
  }, []);

  useEffect(() => {
    const modal = modalRef.current;
    if (!modal) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key !== 'Tab') return;
      const focusable = modal.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      );
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };

    modal.addEventListener('keydown', handleKeyDown);
    return () => modal.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div ref={modalRef} className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" role="dialog" aria-modal="true" aria-labelledby="keyboard-shortcuts-title" onClick={onClose}>
      <div className="bg-bg-secondary border border-border rounded-xl w-full max-w-sm shadow-2xl" onClick={e => e.stopPropagation()}>
        <div className="px-6 py-4 border-b border-border flex items-center justify-between">
          <h2 id="keyboard-shortcuts-title" className="text-lg font-medium text-white">{t('shortcuts.title')}</h2>
          <button
            ref={closeButtonRef}
            onClick={onClose}
            aria-label={t('shortcuts.close')}
            className="w-8 h-8 rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border flex items-center justify-center transition-all"
          >
            &times;
          </button>
        </div>
        <div className="p-6 space-y-4">
          {/* Navigation */}
          <div>
            <div className="text-[10px] uppercase tracking-wider text-text-muted mb-2">{t('shortcuts.navigateItems')}</div>
            <div className="space-y-2">
              {[
                { key: 'j', label: t('shortcuts.nextItem', 'Next item') },
                { key: 'k', label: t('shortcuts.previousItem', 'Previous item') },
              ].map(({ key, label }) => (
                <div key={key} className="flex items-center justify-between">
                  <kbd className="px-2 py-1 bg-bg-tertiary border border-border rounded text-sm font-mono text-white min-w-[2.5rem] text-center">{key}</kbd>
                  <span className="text-sm text-text-secondary">{label}</span>
                </div>
              ))}
            </div>
          </div>
          {/* Actions */}
          <div>
            <div className="text-[10px] uppercase tracking-wider text-text-muted mb-2">{t('shortcuts.actionsGroup', 'Actions')}</div>
            <div className="space-y-2">
              {[
                { key: 'Enter', label: t('shortcuts.openItem', 'Open item') },
                { key: 's', label: t('shortcuts.saveItem') },
                { key: 'd', label: t('shortcuts.dismissItem', 'Dismiss item') },
                { key: 'R', label: t('shortcuts.runAnalysis') },
              ].map(({ key, label }) => (
                <div key={key} className="flex items-center justify-between">
                  <kbd className="px-2 py-1 bg-bg-tertiary border border-border rounded text-sm font-mono text-white min-w-[2.5rem] text-center">{key}</kbd>
                  <span className="text-sm text-text-secondary">{label}</span>
                </div>
              ))}
            </div>
          </div>
          {/* Views & Panels */}
          <div>
            <div className="text-[10px] uppercase tracking-wider text-text-muted mb-2">{t('shortcuts.viewsGroup', 'Views & Panels')}</div>
            <div className="space-y-2">
              {[
                { key: '/', label: t('shortcuts.focusSearch', 'Focus search') },
                { key: 'F', label: t('shortcuts.toggleFilter') },
                { key: 'B', label: t('shortcuts.switchView') },
                { key: ',', label: t('shortcuts.openSettings') },
                { key: 'Esc', label: t('shortcuts.closePanel') },
                { key: '?', label: t('shortcuts.showHelp') },
              ].map(({ key, label }) => (
                <div key={key} className="flex items-center justify-between">
                  <kbd className="px-2 py-1 bg-bg-tertiary border border-border rounded text-sm font-mono text-white min-w-[2.5rem] text-center">{key}</kbd>
                  <span className="text-sm text-text-secondary">{label}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
