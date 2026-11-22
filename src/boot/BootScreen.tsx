// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The first thing the window shows: 4DA's mark, in motion, while the app's
 * code loads. Rendered by main.tsx before the dynamic App import, so it can
 * use nothing that import brings: no App.css, no Tailwind reset, no i18next.
 *
 * It is drawn from the splash screen's own style objects, in the same
 * arrangement, at the splash's first stage. The splash screen takes over the
 * moment the app's code arrives, and because the two are laid out identically
 * the person sees one start-up screen moving forward, not two screens.
 * Until 2026-11-22 this was a single grey line ("Loading 4DA") with nothing
 * moving, which read as a hang whenever the load took more than a moment.
 */
import { useMemo } from 'react';
import type { CSSProperties } from 'react';
import sunLogo from '../assets/sun-logo.webp';
import sunLogoLight from '../assets/sun-logo-light.webp';
import {
  splashKeyframes,
  containerStyle,
  logoRingStyle,
  spinnerRingStyle,
  brandNameStyle,
  taglineStyle,
  progressTrackStyle,
  progressFillStyle,
  miniSpinnerStyle,
  stageOrder,
} from '../components/splash/splash-styles';
import { useBootElapsedSeconds } from './boot-clock';
import { bootStrings } from './boot-strings';
import { bootTokenStyle, isLightTheme } from './boot-theme';
import { SlowStartNotice } from './SlowStartNotice';

const BRAND = '4DA';
const NBSP = ' ';

/**
 * What App.css and the Tailwind reset will supply once they load. Without
 * these the boot screen would be measured differently from the splash screen
 * that replaces it (content-box sizing, browser heading margins, a serif
 * default font) and the hand-over would visibly jump.
 */
const BOOT_CSS = `
.fourda-boot, .fourda-boot * { box-sizing: border-box; }
.fourda-boot h1, .fourda-boot p { margin: 0; }
.fourda-boot img { display: block; }
.fourda-boot button { font: inherit; }
@media (prefers-reduced-motion: reduce) {
  .fourda-boot * { animation: none !important; }
}
${splashKeyframes}
`;

const FONT_STACK = "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

const FAILED_TITLE: CSSProperties = {
  fontSize: '0.9375rem',
  fontWeight: 500,
  color: 'var(--color-error)',
};

const FAILED_HINT: CSSProperties = {
  marginTop: '0.5rem',
  maxWidth: '24rem',
  textAlign: 'center',
  fontSize: '0.8125rem',
  color: 'var(--color-text-secondary)',
};

const RETRY: CSSProperties = {
  marginTop: '1.25rem',
  padding: '0.5rem 1.1rem',
  fontSize: '0.875rem',
  fontWeight: 500,
  color: '#FFFFFF',
  backgroundColor: 'var(--color-accent-action)',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
};

interface BootScreenProps {
  /** `loading` while the app's code is on its way; `failed` when it could not be loaded. */
  phase: 'loading' | 'failed';
  /** Development build (the interface comes from the dev server). */
  devBuild: boolean;
  onReload?: () => void;
}

export function BootScreen({
  phase,
  devBuild,
  onReload = () => window.location.reload(),
}: BootScreenProps) {
  const elapsedSeconds = useBootElapsedSeconds();
  const strings = useMemo(() => bootStrings(), []);
  const light = isLightTheme();
  const failed = phase === 'failed';
  // The splash screen's first stage: one step of its progress, one dot lit.
  const progress = (1 / stageOrder.length) * 100;

  return (
    <div
      className="fourda-boot"
      data-testid="boot-screen"
      data-phase={phase}
      aria-busy={!failed}
      style={{
        ...containerStyle(false),
        ...bootTokenStyle(light),
        fontFamily: FONT_STACK,
        lineHeight: 1.5,
        color: 'var(--color-text-primary)',
      }}
    >
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
        <div style={{ position: 'relative', marginBottom: '2rem' }}>
          <div style={failed ? { ...logoRingStyle, animation: 'none' } : logoRingStyle}>
            <img
              src={light ? sunLogoLight : sunLogo}
              alt={BRAND}
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
            />
          </div>
          {!failed && <div style={spinnerRingStyle} data-testid="boot-spinner" />}
        </div>

        <h1 style={brandNameStyle}>{BRAND}</h1>

        {failed ? (
          <>
            <p style={{ ...FAILED_TITLE, marginTop: '1rem' }} role="alert">{strings.failed}</p>
            <p style={FAILED_HINT}>{strings.failedHint}</p>
            <button type="button" style={RETRY} onClick={onReload}>
              {strings.retry}
            </button>
          </>
        ) : (
          <>
            {/* The tagline arrives with the splash screen (it is translated
                there); its space is held so nothing moves when it does. */}
            <p style={taglineStyle} aria-hidden="true">{NBSP}</p>

            <div style={progressTrackStyle}>
              <div style={progressFillStyle(progress)} />
            </div>
            <div style={{ height: '0.75rem' }} />

            <div style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: '0.25rem',
              minHeight: '40px',
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                <div style={miniSpinnerStyle} />
                <span role="status" style={{ fontSize: '0.875rem', color: 'var(--color-text-secondary)' }}>
                  {strings.starting}
                </span>
              </div>
              <span style={{ fontSize: '0.6875rem' }} aria-hidden="true">{NBSP}</span>
            </div>

            <div style={{ display: 'flex', gap: '0.5rem', marginTop: '1.5rem' }} aria-hidden="true">
              {stageOrder.slice(0, -1).map((stage, i) => (
                <div
                  key={stage}
                  style={{
                    width: '8px',
                    height: '8px',
                    borderRadius: '50%',
                    backgroundColor: i === 0 ? 'var(--color-accent-action)' : 'var(--color-border)',
                  }}
                />
              ))}
            </div>

            <SlowStartNotice
              elapsedSeconds={elapsedSeconds}
              strings={strings}
              devBuild={devBuild}
              showReload
              onReload={onReload}
            />
          </>
        )}
      </div>

      <style>{BOOT_CSS}</style>
    </div>
  );
}
