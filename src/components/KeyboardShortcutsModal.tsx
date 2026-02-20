import { useEffect, useRef } from 'react';

interface KeyboardShortcutsModalProps {
  onClose: () => void;
}

export function KeyboardShortcutsModal({ onClose }: KeyboardShortcutsModalProps) {
  const closeButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
    return () => {
      const trigger = document.querySelector<HTMLElement>('[data-shortcut-trigger]');
      trigger?.focus();
    };
  }, []);

  return (
    <div className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4" role="dialog" aria-modal="true" aria-labelledby="keyboard-shortcuts-title" onClick={onClose}>
      <div className="bg-bg-secondary border border-border rounded-xl w-full max-w-sm shadow-2xl" onClick={e => e.stopPropagation()}>
        <div className="px-6 py-4 border-b border-border flex items-center justify-between">
          <h2 id="keyboard-shortcuts-title" className="text-lg font-medium text-white">Keyboard Shortcuts</h2>
          <button
            ref={closeButtonRef}
            onClick={onClose}
            aria-label="Close keyboard shortcuts"
            className="w-8 h-8 rounded-lg bg-bg-tertiary text-gray-500 hover:text-white hover:bg-border flex items-center justify-center transition-all"
          >
            &times;
          </button>
        </div>
        <div className="p-6 space-y-3">
          {[
            { key: 'R', label: 'Run analysis' },
            { key: 'F', label: 'Toggle relevant-only filter' },
            { key: 'B', label: 'Switch view (Intelligence / Results)' },
            { key: ',', label: 'Open settings' },
            { key: 'Ctrl+`', label: 'Toggle Command Deck' },
            { key: 'Esc', label: 'Close panel / modal' },
            { key: '?', label: 'Show this help' },
          ].map(({ key, label }) => (
            <div key={key} className="flex items-center justify-between">
              <kbd className="px-2 py-1 bg-bg-tertiary border border-border rounded text-sm font-mono text-white min-w-[2.5rem] text-center">
                {key}
              </kbd>
              <span className="text-sm text-text-secondary">{label}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
