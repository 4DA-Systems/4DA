// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { Trans, useTranslation } from 'react-i18next';

export function BrowserModeNotice() {
  const { t } = useTranslation();

  return (
    <div className="mb-6 px-4 py-4 bg-bg-secondary border border-border rounded-lg">
      <p className="text-sm font-medium text-text-primary mb-2">{t('browser.title')}</p>
      <p className="text-xs text-gray-400">
        {t('browser.description')}
      </p>
      <p className="text-xs text-gray-500 mt-2">
        <Trans
          i18nKey="browser.hint"
          components={{
            code: <code className="font-mono text-gray-400" />,
          }}
        />
      </p>
    </div>
  );
}
