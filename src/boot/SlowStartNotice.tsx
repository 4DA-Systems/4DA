// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * What a start-up screen says once the wait stops being ordinary.
 *
 * A screen that sits unchanged looks hung, and a person who thinks an app has
 * hung closes it (operator report 2026-11-22: three minutes on a bare
 * "Loading 4DA"). So after a few seconds the screen says the start is still
 * going and shows a running count, which is the proof it has not frozen. After
 * longer it says the likely reason and offers a reload.
 *
 * It states only what is known. A development build knows why it is slow (the
 * dev server compiles the interface on first request) and can show the files
 * arriving; an installed build knows only that the start is slow, and names
 * the common cause without claiming it.
 *
 * Shared by the boot screen and the splash screen; both read the same clock,
 * so the count carries across the hand-over between them.
 */
import type { CSSProperties } from 'react';
import { formatElapsed, loadedFileCount, startPace } from './boot-clock';
import type { BootStrings } from './boot-strings';

// Development builds only; never shown to someone running an installed 4DA.
const DEV_NOTE =
  'Development build: the dev server is compiling the interface. The first load after a restart can take a few minutes.';
const devFileCount = (count: number) => `${count} files loaded so far.`;

/** Reserved whether or not there is anything to say, so the screen above never jumps. */
const AREA: CSSProperties = {
  marginTop: '1.25rem',
  minHeight: '9rem',
  maxWidth: '26rem',
  padding: '0 1rem',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: '0.375rem',
  textAlign: 'center',
  fontSize: '0.875rem',
  lineHeight: 1.5,
  color: 'var(--color-text-secondary)',
};

const COUNT: CSSProperties = {
  marginInlineStart: '0.5rem',
  color: 'var(--color-text-muted)',
  fontVariantNumeric: 'tabular-nums',
};

const DETAIL: CSSProperties = {
  margin: 0,
  fontSize: '0.8125rem',
  color: 'var(--color-text-muted)',
};

const RELOAD: CSSProperties = {
  marginTop: '0.5rem',
  padding: '0.4rem 0.9rem',
  fontSize: '0.8125rem',
  color: 'var(--color-text-primary)',
  backgroundColor: 'transparent',
  border: '1px solid var(--color-border)',
  borderRadius: '6px',
  cursor: 'pointer',
};

interface SlowStartNoticeProps {
  elapsedSeconds: number;
  strings: BootStrings;
  /** Development build: say so, and show the files arriving. */
  devBuild: boolean;
  /** Offer a reload once the start is very slow. Off where the screen already has one. */
  showReload: boolean;
  onReload?: () => void;
}

export function SlowStartNotice({
  elapsedSeconds,
  strings,
  devBuild,
  showReload,
  onReload = () => window.location.reload(),
}: SlowStartNoticeProps) {
  const pace = startPace(elapsedSeconds);
  const slow = pace !== 'normal';
  const verySlow = pace === 'verySlow';

  return (
    <div style={AREA} data-testid="slow-start-notice" data-pace={pace}>
      {slow && (
        <p style={{ margin: 0 }}>
          {/* Only the sentence is announced: a count that changes every
              second would be read out every second. */}
          <span role="status">{strings.slow}</span>
          <span style={COUNT} aria-hidden="true">{formatElapsed(elapsedSeconds)}</span>
        </p>
      )}
      {slow && devBuild && (
        <p style={DETAIL}>
          {DEV_NOTE}{' '}
          <span aria-hidden="true">{devFileCount(loadedFileCount())}</span>
        </p>
      )}
      {verySlow && !devBuild && (
        <p style={DETAIL} role="status">{strings.verySlow}</p>
      )}
      {verySlow && showReload && (
        <button type="button" style={RELOAD} onClick={onReload}>
          {strings.reload}
        </button>
      )}
    </div>
  );
}
