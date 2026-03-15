// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useTranslation } from 'react-i18next';

interface UpdateInfo {
  version: string;
  body?: string | null;
  canAutoUpdate?: boolean;
}

interface UpdateBannerProps {
  update: UpdateInfo;
  installing: boolean;
  onInstall: () => void;
  onDismiss: () => void;
}

export function UpdateBanner({ update, installing, onInstall, onDismiss }: UpdateBannerProps) {
  const { t } = useTranslation();

  return (
    <div className="fixed bottom-4 right-4 z-50 bg-bg-secondary border border-[#D4AF37]/40 rounded-xl px-5 py-4 shadow-lg max-w-sm">
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <p className="text-sm font-medium text-white">{t('update.available', { version: update.version })}</p>
          <p className="text-xs text-text-secondary mt-1">
            {update.body ? update.body.slice(0, 100) : t('update.defaultBody')}
          </p>
        </div>
        <button onClick={onDismiss} aria-label="Dismiss update notification" className="text-text-muted hover:text-white text-lg leading-none">&times;</button>
      </div>
      <div className="flex gap-2 mt-3">
        {update.canAutoUpdate !== false ? (
          <button
            onClick={onInstall}
            disabled={installing}
            aria-label={installing ? t('update.installing') : t('update.install')}
            className="px-4 py-1.5 text-xs font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
          >
            {installing ? t('update.installing') : t('update.install')}
          </button>
        ) : (
          <a
            href={`https://github.com/runyourempire/4DA/releases/latest`}
            target="_blank"
            rel="noopener noreferrer"
            className="px-4 py-1.5 text-xs font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors inline-block"
          >
            {t('update.download', 'Download Update')}
          </a>
        )}
        <button
          onClick={onDismiss}
          aria-label={t('update.later')}
          className="px-4 py-1.5 text-xs text-text-secondary hover:text-white transition-colors"
        >
          {t('update.later')}
        </button>
      </div>
    </div>
  );
}
