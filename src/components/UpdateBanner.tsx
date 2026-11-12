// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useTranslation } from 'react-i18next';
import type { InstallFailure, UpdateProgress } from '../hooks/use-update-check';

interface UpdateInfo {
  version: string;
  body?: string | null;
  canAutoUpdate?: boolean;
}

interface UpdateBannerProps {
  update: UpdateInfo;
  installing: boolean;
  progress?: UpdateProgress | null;
  installError?: InstallFailure | null;
  onInstall: () => void;
  onDismiss: () => void;
}

const mb = (bytes: number) => (bytes / 1_048_576).toFixed(1);

export function UpdateBanner({ update, installing, progress, installError, onInstall, onDismiss }: UpdateBannerProps) {
  const { t } = useTranslation();
  const percent = progress?.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : null;

  return (
    <div className="fixed bottom-4 end-4 z-50 bg-bg-secondary border border-accent-gold/40 rounded-xl px-5 py-4 shadow-lg max-w-sm">
      <div className="flex items-start gap-3">
        <div className="flex-1">
          <p className="text-sm font-medium text-text-primary">{t('update.available', { version: update.version })}</p>
          <p className="text-xs text-text-secondary mt-1">
            {update.body ? update.body.slice(0, 100) : t('update.defaultBody')}
          </p>
        </div>
        <button onClick={onDismiss} aria-label="Dismiss update notification" className="text-text-muted hover:text-text-primary text-lg leading-none">&times;</button>
      </div>

      {installing && progress && (
        <div className="mt-3" data-testid="update-progress">
          <div
            className="h-1.5 w-full rounded-full bg-bg-tertiary overflow-hidden"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent ?? undefined}
            aria-label={t('update.installing')}
          >
            <div className="h-full bg-accent-gold transition-all" style={{ width: `${percent ?? 0}%` }} />
          </div>
          <p className="text-[11px] text-text-muted mt-1">
            {progress.total
              ? t('update.progress', { done: mb(progress.downloaded), total: mb(progress.total) })
              : t('update.progressUnknown', { done: mb(progress.downloaded) })}
          </p>
        </div>
      )}

      {installError && (
        <p role="alert" className="text-xs text-error mt-3">
          {installError.stage === 'restart'
            ? t('update.restartFailed', { error: installError.message })
            : t('update.installFailed', { error: installError.message })}
        </p>
      )}

      <div className="flex gap-2 mt-3">
        {update.canAutoUpdate !== false ? (
          installError?.stage === 'restart' ? null : (
            <button
              onClick={onInstall}
              disabled={installing}
              aria-label={installing ? t('update.installing') : installError ? t('update.retry') : t('update.install')}
              className="px-4 py-1.5 text-xs font-medium text-bg-primary bg-accent-gold rounded-lg hover:bg-accent-gold-hover transition-colors disabled:opacity-50"
            >
              {installing ? t('update.installing') : installError ? t('update.retry') : t('update.install')}
            </button>
          )
        ) : (
          <a
            href={`https://github.com/4DA-Systems/4DA/releases/latest`}
            target="_blank"
            rel="noopener noreferrer"
            className="px-4 py-1.5 text-xs font-medium text-bg-primary bg-accent-gold rounded-lg hover:bg-accent-gold-hover transition-colors inline-block"
          >
            {t('update.download', 'Download Update')}
          </a>
        )}
        <button
          onClick={onDismiss}
          aria-label={t('update.later')}
          className="px-4 py-1.5 text-xs text-text-secondary hover:text-text-primary transition-colors"
        >
          {t('update.later')}
        </button>
      </div>
    </div>
  );
}
