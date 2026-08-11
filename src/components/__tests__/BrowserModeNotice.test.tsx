// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('react-i18next', async () => {
  const React = await import('react');
  return {
    useTranslation: () => ({
      t: (key: string) => ({
        'browser.title': 'Desktop App Required',
        'browser.description': '4DA runs as a desktop app.',
      })[key] ?? key,
    }),
    Trans: ({ components, i18nKey }: {
      components?: { code?: React.ReactElement };
      i18nKey: string;
    }) => {
      if (i18nKey !== 'browser.hint') return i18nKey;
      const code = components?.code;
      return (
        <>
          Run {React.isValidElement(code) ? React.cloneElement(code, {}, 'npm run tauri dev') : 'npm run tauri dev'} or launch the installed app.
        </>
      );
    },
  };
});

import { BrowserModeNotice } from '../BrowserModeNotice';

describe('BrowserModeNotice', () => {
  it('renders the desktop launch command as code instead of literal translation markup', () => {
    const { container } = render(<BrowserModeNotice />);

    expect(screen.getByText('Desktop App Required')).toBeVisible();
    expect(container).not.toHaveTextContent('<code>');
    expect(container).not.toHaveTextContent('</code>');

    const command = screen.getByText('npm run tauri dev');
    expect(command.tagName).toBe('CODE');
  });
});
