// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * LanguageSwitcher — the single canonical UI-language picker.
 *
 * Every surface that lets the user change the *app* language should use this
 * component instead of re-implementing the change+persist path. It guarantees:
 *   1. The offered languages match i18n's shipped set (SUPPORTED_LANGUAGES) —
 *      no more offering hi/ar/it that the app cannot fully render.
 *   2. The canonical persistence path runs on every change:
 *        - i18n.changeLanguage(code)         → instant app-wide re-render
 *        - localStorage[LANGUAGE_STORAGE_KEY] → i18n reads this on next boot
 *        - cmd('set_language', ...)           → backend Rust strings follow
 *      (ContentTranslationProvider also pushes set_language on i18n.language
 *       change; doing it here too keeps the backend correct even if that
 *       provider isn't mounted yet, e.g. during onboarding.)
 *
 * Compact globe + dropdown, suitable for pinning in a corner.
 */

import { memo, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../lib/commands';
import { LANGUAGE_STORAGE_KEY, SUPPORTED_LANGUAGES } from '../i18n';

interface LanguageSwitcherProps {
  /** Optional extra classes for the trigger button. */
  className?: string;
}

export const LanguageSwitcher = memo(function LanguageSwitcher({ className = '' }: LanguageSwitcherProps) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close on outside click / Escape so the floating menu never traps the user.
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onClick);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onClick);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const current =
    SUPPORTED_LANGUAGES.find((l) => l.code === i18n.language) ??
    // Fall back by base language (e.g. "pt-BR" stored, "pt" active) then English.
    SUPPORTED_LANGUAGES.find((l) => l.code === i18n.language.split('-')[0]) ??
    SUPPORTED_LANGUAGES[0]!;

  const changeLanguage = (code: string) => {
    void i18n.changeLanguage(code);
    try {
      localStorage.setItem(LANGUAGE_STORAGE_KEY, code);
    } catch {
      /* localStorage unavailable — i18n.changeLanguage still applies for the session */
    }
    // Keep backend Rust-generated strings in sync; preserves country/currency.
    void cmd('set_language', { language: code }).catch(() => {});
    setOpen(false);
  };

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-label={t('language.change', 'Change language')}
        aria-haspopup="listbox"
        aria-expanded={open}
        className={`flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-text-secondary bg-bg-secondary/80 backdrop-blur border border-border rounded-lg hover:border-orange-500/30 hover:text-text-primary transition-all ${className}`}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <circle cx="12" cy="12" r="10" />
          <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
        </svg>
        <span>{current.native}</span>
      </button>

      {open && (
        <div
          role="listbox"
          aria-label={t('language.change', 'Change language')}
          className="absolute start-0 mt-1 w-40 bg-bg-secondary border border-border rounded-lg shadow-xl z-50 max-h-64 overflow-y-auto"
        >
          {SUPPORTED_LANGUAGES.map((lang) => (
            <button
              key={lang.code}
              type="button"
              role="option"
              aria-selected={current.code === lang.code}
              onClick={() => changeLanguage(lang.code)}
              className={`w-full text-start px-3 py-2 text-sm hover:bg-bg-tertiary transition-colors ${
                current.code === lang.code ? 'text-orange-400 font-medium' : 'text-text-secondary'
              }`}
            >
              {lang.native}
            </button>
          ))}
        </div>
      )}
    </div>
  );
});
