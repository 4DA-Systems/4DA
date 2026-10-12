// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PrivacySection } from './PrivacySection';

let level = 'full';
let githubNotes = false;
const cmdMock = vi.fn((name: string, _args?: unknown): Promise<unknown> => {
  if (name === 'get_privacy_config') {
    return Promise.resolve({
      llm_content_level: level,
      proxy_url: null,
      cloud_llm_disclosure_accepted: false,
      activity_tracking_opt_in: false,
      github_release_notes: githubNotes,
    });
  }
  return Promise.resolve(undefined);
});
vi.mock('../../lib/commands', () => ({
  cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])),
}));

beforeEach(() => {
  cmdMock.mockClear();
  level = 'full';
  githubNotes = false;
});

// The test i18n setup renders keys, not the inline English defaults. The label
// itself carries the aria-label (this component's existing pattern), so wait for
// it, then take the checkbox it wraps.
const titlesOnlyBox = async (): Promise<HTMLInputElement> => {
  await screen.findByLabelText('settings.privacy.titlesOnly.title');
  const box = document.getElementById('privacy-titles-only');
  if (!(box instanceof HTMLInputElement)) throw new Error('titles-only checkbox not rendered');
  return box;
};

describe('PrivacySection — titles only (NETWORK.md "Settings → Privacy")', () => {
  it('reflects the stored level', async () => {
    level = 'titles_only';
    render(<PrivacySection />);
    expect(await titlesOnlyBox()).toBeChecked();
  });

  it('is off by default', async () => {
    render(<PrivacySection />);
    expect(await titlesOnlyBox()).not.toBeChecked();
  });

  it('persists titles_only and back to full', async () => {
    render(<PrivacySection />);
    fireEvent.click(await titlesOnlyBox());
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_privacy_config', { llmContentLevel: 'titles_only' }),
    );
    await waitFor(async () => expect(await titlesOnlyBox()).toBeChecked());

    fireEvent.click(await titlesOnlyBox());
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_privacy_config', { llmContentLevel: 'full' }),
    );
  });

  it('stays where it was when saving fails', async () => {
    render(<PrivacySection />);
    const box = await titlesOnlyBox();
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('boom')));
    fireEvent.click(box);
    await waitFor(() => expect(box).not.toBeDisabled());
    expect(box).not.toBeChecked();
  });
});

const githubNotesBox = async (): Promise<HTMLInputElement> => {
  await screen.findByLabelText('settings.privacy.githubReleaseNotes.title');
  const box = document.getElementById('privacy-github-release-notes');
  if (!(box instanceof HTMLInputElement)) throw new Error('GitHub release-notes checkbox not rendered');
  return box;
};

describe('PrivacySection — GitHub release notes (NETWORK.md §2m, opt-in)', () => {
  it('is off by default and says what GitHub sees', async () => {
    render(<PrivacySection />);
    expect(await githubNotesBox()).not.toBeChecked();
    expect(screen.getByText('settings.privacy.githubReleaseNotes.subtitle')).toBeTruthy();
  });

  it('reflects the stored opt-in', async () => {
    githubNotes = true;
    render(<PrivacySection />);
    expect(await githubNotesBox()).toBeChecked();
  });

  it('persists the opt-in and the opt-out', async () => {
    render(<PrivacySection />);
    fireEvent.click(await githubNotesBox());
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_privacy_config', { githubReleaseNotes: true }),
    );
    await waitFor(async () => expect(await githubNotesBox()).toBeChecked());

    fireEvent.click(await githubNotesBox());
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('set_privacy_config', { githubReleaseNotes: false }),
    );
    await waitFor(async () => expect(await githubNotesBox()).not.toBeChecked());
  });

  it('stays off when saving fails', async () => {
    render(<PrivacySection />);
    const box = await githubNotesBox();
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('boom')));
    fireEvent.click(box);
    await waitFor(() => expect(box).not.toBeDisabled());
    expect(box).not.toBeChecked();
  });
});
