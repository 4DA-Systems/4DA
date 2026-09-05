// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => (typeof fallback === 'string' ? fallback : key),
  }),
}));

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));

const setShowSettings = vi.fn();
vi.mock('../store', () => ({
  useAppStore: (selector: (s: { setShowSettings: () => void }) => unknown) =>
    selector({ setShowSettings }),
}));

vi.mock('./geometry/LogoMarkSVG', () => ({ LogoMarkSVG: () => <svg data-testid="logo" /> }));
vi.mock('./geometry/GeometryShowcase', () => ({ GeometryShowcase: () => null }));
vi.mock('./settings/PrivacySection', () => ({ PrivacySection: () => null }));

import { AboutPanel } from './AboutPanel';

describe('AboutPanel', () => {
  it('quits through the quit_app command — the same path as the tray menu', () => {
    vi.stubGlobal('__APP_VERSION__', '0.0.0-test');
    render(<AboutPanel />);
    expect(cmdMock).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('about.quitApp'));
    expect(cmdMock).toHaveBeenCalledTimes(1);
    expect(cmdMock).toHaveBeenCalledWith('quit_app');
  });
});
