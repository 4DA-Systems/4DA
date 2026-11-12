// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useTranslation } from 'react-i18next';
import { canCheckForUpdates, requestUpdateCheck, useUpdateStatus } from '../hooks/update-status';

/**
 * When the app last looked for an update, and why it could not, in the About
 * panel. A check that fails (offline, proxy, GitHub down) is said here rather
 * than looking like "up to date". Absent in dev builds, which have no updater.
 */
export function UpdateCheckStatus() {
  const { t } = useTranslation();
  const status = useUpdateStatus();
  if (!canCheckForUpdates()) return null;

  const when = status.lastCheckedAt ? new Date(status.lastCheckedAt).toLocaleString() : null;
  let line: string;
  if (status.checking) line = t('about.updates.checking');
  else if (status.lastError) line = t('about.updates.failed', { error: status.lastError, time: when ?? '' });
  else if (when) line = t('about.updates.lastChecked', { time: when });
  else line = t('about.updates.never');

  return (
    <div className="flex items-center justify-center gap-2 mt-1.5" data-testid="update-check-status">
      <p className={`text-[10px] ${status.lastError && !status.checking ? 'text-error' : 'text-text-muted'}`}>{line}</p>
      <button
        type="button"
        disabled={status.checking}
        onClick={() => {
          void requestUpdateCheck();
        }}
        className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2 disabled:opacity-50"
      >
        {t('about.updates.checkNow')}
      </button>
    </div>
  );
}
