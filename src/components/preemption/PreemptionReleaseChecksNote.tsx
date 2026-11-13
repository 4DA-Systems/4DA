// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import type { AdvisorySync } from '../../../src-tauri/bindings/bindings/AdvisorySync';

/**
 * Release checks 4DA did not make (audit P1): Settings → Privacy → Registry
 * lookups is off, or some dependencies have private names that are never
 * sent to a public registry. Advisories are still matched either way, so this
 * note states what is not checked instead of changing the all-clear.
 */
export const PreemptionReleaseChecksNote = memo(function PreemptionReleaseChecksNote({
  sync,
}: {
  sync: AdvisorySync | null | undefined;
}) {
  const { t } = useTranslation();
  const off = sync?.registry_lookups_off === true;
  const privateCount = sync?.private_not_looked_up ?? 0;
  if (!off && privateCount === 0) return null;
  return (
    <div
      className="px-4 py-3 rounded-lg bg-bg-secondary border border-border text-xs"
      data-testid="preemption-release-checks"
      role="note"
    >
      {off ? (
        <>
          <p className="font-medium text-text-primary mb-1">{t('preemption.releaseChecks.offTitle')}</p>
          <p className="text-text-muted">{t('preemption.releaseChecks.offBody')}</p>
        </>
      ) : (
        <>
          <p className="font-medium text-text-primary mb-1">
            {t('preemption.releaseChecks.privateTitle', { count: privateCount })}
          </p>
          <p className="text-text-muted">{t('preemption.releaseChecks.privateBody')}</p>
        </>
      )}
    </div>
  );
});
