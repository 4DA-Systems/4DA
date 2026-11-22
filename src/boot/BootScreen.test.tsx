// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { BootScreen } from './BootScreen';
import { SLOW_AFTER_SECONDS, VERY_SLOW_AFTER_SECONDS } from './boot-clock';
import { BOOT_TOKENS } from './boot-theme';
import en from '../locales/en/boot.json';

/** The boot clock is `performance.now()`: put the page `seconds` into its start. */
function atSeconds(seconds: number) {
  vi.spyOn(performance, 'now').mockReturnValue(seconds * 1000);
}

afterEach(() => {
  vi.restoreAllMocks();
  document.documentElement.removeAttribute('data-theme');
});

describe('BootScreen while loading', () => {
  it('shows the mark in motion and says 4DA is starting, with nothing alarming', () => {
    atSeconds(1);
    render(<BootScreen phase="loading" devBuild={false} />);

    expect(screen.getByText(en.starting)).toBeInTheDocument();
    expect(screen.getByTestId('boot-spinner')).toBeInTheDocument();
    expect(screen.getByTestId('slow-start-notice')).toHaveAttribute('data-pace', 'normal');
    expect(screen.queryByText(en.slow)).not.toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('says the start is still going, with a running count, once it is slow', () => {
    atSeconds(SLOW_AFTER_SECONDS + 1);
    render(<BootScreen phase="loading" devBuild={false} />);

    expect(screen.getByText(en.slow)).toBeInTheDocument();
    expect(screen.getByText('0:07')).toBeInTheDocument();
    // Not yet the point to speculate about causes or offer a reload.
    expect(screen.queryByText(en.verySlow)).not.toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('names the likely cause and offers a reload once it is very slow', () => {
    atSeconds(VERY_SLOW_AFTER_SECONDS + 55);
    const onReload = vi.fn();
    render(<BootScreen phase="loading" devBuild={false} onReload={onReload} />);

    expect(screen.getByText(en.slow)).toBeInTheDocument();
    expect(screen.getByText(en.verySlow)).toBeInTheDocument();
    expect(screen.getByText('1:15')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: en.reload }));
    expect(onReload).toHaveBeenCalledTimes(1);
  });

  it('a development build says it is the dev server, not the computer starting', () => {
    atSeconds(VERY_SLOW_AFTER_SECONDS + 1);
    render(<BootScreen phase="loading" devBuild />);

    expect(screen.getByText(/Development build: the dev server is compiling/)).toBeInTheDocument();
    expect(screen.getByText(/files loaded so far/)).toBeInTheDocument();
    expect(screen.queryByText(en.verySlow)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: en.reload })).toBeInTheDocument();
  });

  it('never shows the development note in an installed build', () => {
    atSeconds(VERY_SLOW_AFTER_SECONDS + 1);
    render(<BootScreen phase="loading" devBuild={false} />);

    expect(screen.queryByText(/Development build/)).not.toBeInTheDocument();
  });
});

describe('BootScreen when the app could not be loaded', () => {
  it('says so, stops the motion, and offers a retry', () => {
    atSeconds(3);
    const onReload = vi.fn();
    render(<BootScreen phase="failed" devBuild={false} onReload={onReload} />);

    expect(screen.getByRole('alert')).toHaveTextContent(en.failed);
    expect(screen.getByText(en.failedHint)).toBeInTheDocument();
    expect(screen.queryByTestId('boot-spinner')).not.toBeInTheDocument();
    expect(screen.queryByText(en.starting)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: en.retry }));
    expect(onReload).toHaveBeenCalledTimes(1);
  });
});

describe('BootScreen theme', () => {
  it('carries its own design tokens, for whichever theme index.html applied', () => {
    atSeconds(1);
    const { unmount } = render(<BootScreen phase="loading" devBuild={false} />);
    expect(screen.getByTestId('boot-screen').style.getPropertyValue('--color-bg-primary'))
      .toBe(BOOT_TOKENS.dark['--color-bg-primary']);
    unmount();

    document.documentElement.setAttribute('data-theme', 'light');
    render(<BootScreen phase="loading" devBuild={false} />);
    expect(screen.getByTestId('boot-screen').style.getPropertyValue('--color-bg-primary'))
      .toBe(BOOT_TOKENS.light['--color-bg-primary']);
  });
});
