// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
// NOTICE file imported at build time via Vite's ?raw loader.
//
// Previously this component hand-maintained a string constant that mirrored
// the repo's NOTICE file. That mirror drifted — the on-disk NOTICE listed
// ~50 deps while the in-app modal listed ~25. Self-audit 4.8 called out
// that drift as a compliance risk for Apache 2.0 §4(d) attribution and the
// FSL-1.1-Apache-2.0 transparency promise.
//
// The raw-import means: whatever is in NOTICE at build time IS what the
// modal renders. There is no second source of truth to keep in sync.
import NOTICE_TEXT from '../../NOTICE?raw';

interface Props {
  onClose: () => void;
}

export function ThirdPartyLicensesModal({ onClose }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onClose]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(NOTICE_TEXT);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API can fail in some contexts — silently ignore
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="third-party-licenses-title"
    >
      <button
        type="button"
        aria-label={t('about.close', 'Close')}
        className="absolute inset-0 bg-black/70 backdrop-blur-sm border-0 cursor-default"
        onClick={onClose}
      />
      <div className="relative w-full max-w-3xl max-h-[85vh] bg-bg-secondary border border-border rounded-xl shadow-2xl flex flex-col">
        <div className="flex items-center justify-between px-5 py-3 border-b border-border">
          <h2
            id="third-party-licenses-title"
            className="text-sm font-semibold text-white"
          >
            {t('about.thirdPartyLicenses', 'Third-Party Licenses')}
          </h2>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleCopy}
              className="text-xs px-2.5 py-1 text-text-secondary bg-bg-tertiary border border-border rounded hover:bg-bg-primary transition-colors"
            >
              {copied ? t('about.copied', 'Copied') : t('about.copyToClipboard', 'Copy')}
            </button>
            <button
              type="button"
              onClick={onClose}
              className="text-xs px-2.5 py-1 text-text-secondary bg-bg-tertiary border border-border rounded hover:bg-bg-primary transition-colors"
              aria-label={t('about.close', 'Close')}
            >
              {t('about.close', 'Close')}
            </button>
          </div>
        </div>
        <pre className="flex-1 overflow-auto px-5 py-4 text-[11px] leading-relaxed text-text-secondary font-mono whitespace-pre-wrap">
          {NOTICE_TEXT}
        </pre>
      </div>
    </div>
  );
}
