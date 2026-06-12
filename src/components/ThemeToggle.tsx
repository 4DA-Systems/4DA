// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '../lib/theme';

/**
 * Theme toggle — dark (the void) <-> light (paper).
 *
 * Shared between the main app bar and the onboarding wizard (pinned top-right
 * there, mirroring the persistent top-left language switcher), so the choice
 * is offered from the very first screen a user ever sees.
 */
export const ThemeToggle = memo(function ThemeToggle() {
  const { t } = useTranslation();
  const { isLight, toggle } = useTheme();

  const label = isLight
    ? t('header.themeToggleToDark', 'Switch to dark theme')
    : t('header.themeToggleToLight', 'Switch to light theme');

  return (
    <button
      data-theme-toggle
      onClick={toggle}
      className="w-8 h-8 flex items-center justify-center rounded-md bg-bg-secondary text-text-secondary border border-border hover:bg-bg-tertiary hover:border-accent-gold/40 transition-all"
      aria-label={label}
      title={label}
      aria-pressed={isLight}
    >
      {isLight ? (
        /* Moon — offered action: back to the void */
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <path
            d="M12 8.6A5.4 5.4 0 0 1 5.4 2 5.4 5.4 0 1 0 12 8.6Z"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinejoin="round"
          />
        </svg>
      ) : (
        /* Sun — offered action: switch to paper */
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <circle cx="7" cy="7" r="3" stroke="currentColor" strokeWidth="1.3" />
          <path
            d="M7 1v1.4M7 11.6V13M1 7h1.4M11.6 7H13M2.8 2.8l1 1M10.2 10.2l1 1M11.2 2.8l-1 1M3.8 10.2l-1 1"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
          />
        </svg>
      )}
    </button>
  );
});
