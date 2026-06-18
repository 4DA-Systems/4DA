// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { reportError } from '../lib/error-reporter';
import sunLogo from '../assets/sun-logo.webp';
import sunLogoLight from '../assets/sun-logo-light.webp';
import { useTheme } from '../lib/theme';
import { translateError } from '../utils/error-messages';
import {
  type InitStage,
  stageKeys,
  stageExplanations,
  stageOrder,
  splashKeyframes,
  containerStyle,
  logoRingStyle,
  fallbackEmojiStyle,
  spinnerRingStyle,
  brandNameStyle,
  taglineStyle,
  progressTrackStyle,
  progressFillStyle,
  miniSpinnerStyle,
  retryButtonStyle,
  versionStyle,
  refreshButtonStyle,
} from './splash/splash-styles';

interface SplashScreenProps {
  onComplete: () => void;
  minimumDisplayTime?: number;
}

export function SplashScreen({ onComplete, minimumDisplayTime = 800 }: SplashScreenProps) {
  const { t } = useTranslation();
  const { isLight } = useTheme();
  const [fadeOut, setFadeOut] = useState(false);
  const [imageError, setImageError] = useState(false);
  const [stage, setStage] = useState<InitStage>('starting');
  const [backendReady, setBackendReady] = useState(false);
  const [minTimeElapsed, setMinTimeElapsed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Minimum display time
  useEffect(() => {
    const timer = setTimeout(() => {
      setMinTimeElapsed(true);
    }, minimumDisplayTime);
    return () => clearTimeout(timer);
  }, [minimumDisplayTime]);

  // NOTE: frontend-ready is emitted from main.tsx BEFORE React mounts,
  // which is ~300-500ms faster than a useEffect here. No duplicate emit needed.

  // Check backend readiness
  useEffect(() => {
    let cancelled = false;

    const checkBackend = async () => {
      try {
        // Stage 1: Database — must succeed before proceeding.
        // The backend can take far longer than a single command timeout to
        // become IPC-responsive on a cold or loaded start (dev: Vite transform
        // + heavy startup ingestion; prod: a busy machine) — a one-shot
        // get_settings then hard-fails and blocks the app on the splash. Retry
        // with backoff for up to ~90s so the splash rides out a slow backend
        // instead of giving up; a normal start still succeeds on the first try.
        setStage('database');
        {
          // VITEST: single attempt (fail fast) so the error-path test doesn't
          // wait out the retry window. Runtime: retry for up to ~90s.
          const readyDeadline = Date.now() + (import.meta.env.VITEST ? 0 : 90_000);
          let lastErr: unknown;
          let settingsOk = false;
          do {
            try {
              await cmd('get_settings');
              settingsOk = true;
              break;
            } catch (e) {
              lastErr = e;
              if (Date.now() >= readyDeadline) break;
              await new Promise((r) => setTimeout(r, 2000));
            }
          } while (!cancelled);
          if (cancelled) return;
          if (!settingsOk) throw lastErr ?? new Error('backend not ready');
        }

        // Stages 2-4: Parallel non-critical probes with animated stage advancement
        setStage('embeddings');
        await Promise.allSettled([
          cmd('get_context_stats'),
          cmd('get_sources'),
        ]);
        if (cancelled) return;

        // Stage 5: Ready
        setStage('ready');
        setBackendReady(true);

      } catch (e) {
        reportError('SplashScreen.backendCheck', e);
        // Detect browser mode: if Tauri internals are missing, show specific message
        const isBrowser = !('__TAURI_INTERNALS__' in window);
        if (isBrowser) {
          // In real browsers, redirect to Signal Terminal (skip in test/JSDOM)
          if (!import.meta.env.VITEST) {
            const terminalPort = import.meta.env.DEV ? 4445 : 4444;
            window.location.href = `http://localhost:${terminalPort}/`;
            return;
          }
          setError('Desktop app required — open through Tauri window');
        } else {
          setError(translateError(e));
        }
        // Do NOT mark as ready — block the app on database failure.
        // The user can use the refresh button to retry.
      }
    };

    void checkBackend();

    return () => {
      cancelled = true;
    };
  }, []);

  // Transition to app when both conditions are met
  useEffect(() => {
    if (backendReady && minTimeElapsed) {
      setFadeOut(true);
      setTimeout(onComplete, 300);
    }
  }, [backendReady, minTimeElapsed, onComplete]);

  const currentStageIndex = stageOrder.indexOf(stage);
  const progress = ((currentStageIndex + 1) / stageOrder.length) * 100;

  return (
    <div
      role="status"
      aria-label={error ? t('splash.error') : t(stageKeys[stage])}
      aria-busy={stage !== 'ready'}
      style={containerStyle(fadeOut)}
    >
      {/* Content cluster wrapper — ensures the visual block is truly centered
          even with absolutely-positioned elements (version, refresh) outside it */}
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
      {/* Sun Logo with pulse animation */}
      <div style={{ position: 'relative', marginBottom: '2rem' }}>
        <div style={logoRingStyle}>
          {/* eslint-disable i18next/no-literal-string */}
          {!imageError ? (
            /* The dark asset bakes in a black disc; on paper we use the
               luminance-keyed cutout (same treatment as the white merch) */
            <img
              src={isLight ? sunLogoLight : sunLogo}
              alt="4DA"
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
              onError={() => setImageError(true)}
            />
          ) : (
            <div style={fallbackEmojiStyle}>
              ☀️
            </div>
          )}
          {/* eslint-enable i18next/no-literal-string */}
        </div>
        {/* Spinning ring around logo */}
        <div style={spinnerRingStyle} />
      </div>

      {/* Brand Name */}
      {/* eslint-disable i18next/no-literal-string */}
      <h1 style={brandNameStyle}>
        4DA
      </h1>
      {/* eslint-enable i18next/no-literal-string */}

      {/* Tagline */}
      <p style={taglineStyle}>
        {t('app.tagline')}
      </p>

      {/* Progress bar */}
      <div style={progressTrackStyle}>
        <div
          role="progressbar"
          aria-valuenow={Math.round(progress)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={t('splash.progress', { percent: Math.round(progress) })}
          style={progressFillStyle(progress)}
        />
      </div>

      {/* Spacer — no time estimate (progress bar and stages are sufficient context) */}
      {stage !== 'ready' && !error && (
        <div style={{ height: '0.75rem' }} />
      )}

      {/* Status message */}
      <div style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '0.25rem',
        minHeight: '40px',
      }}>
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '0.75rem',
        }}>
        {stage !== 'ready' && !error && (
          <div style={miniSpinnerStyle} />
        )}
        {stage === 'ready' && !error && (
          // eslint-disable-next-line i18next/no-literal-string
          <span style={{ color: 'var(--color-success)', fontSize: '1rem' }}>✓</span>
        )}
        {error && (
          <span style={{ color: 'var(--color-error)', fontSize: '1rem' }}>⚠</span>
        )}
        <span style={{
          fontSize: '0.875rem',
          color: error ? 'var(--color-error)' : stage === 'ready' ? 'var(--color-success)' : 'var(--color-text-secondary)',
          transition: 'color 300ms',
        }}>
          {error || t(stageKeys[stage])}
        </span>
        </div>
        {/* Stage explanation subtitle */}
        {!error && (
          <span style={{
            fontSize: '0.6875rem',
            color: 'var(--color-text-muted)',
            transition: 'opacity 300ms',
          }}>
            {t(`splash.stageExplanation.${stage}`, stageExplanations[stage])}
          </span>
        )}
        {error && (
          <button
            onClick={() => window.location.reload()}
            style={retryButtonStyle}
          >
            {t('action.retry')}
          </button>
        )}
      </div>

      {/* Stage indicators */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        marginTop: '1.5rem',
      }} aria-hidden="true">
        {stageOrder.slice(0, -1).map((s, i) => (
          <div
            key={s}
            style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              backgroundColor: i <= currentStageIndex ? 'var(--color-accent-action)' : 'var(--color-border)',
              transition: 'background-color 300ms',
            }}
          />
        ))}
      </div>
      </div>

      {/* Version */}
      <p style={versionStyle}>
        {t('splash.version', { version: __APP_VERSION__ })}
      </p>

      {/* Subtle refresh button - top right corner */}
      <button
        onClick={() => window.location.reload()}
        style={refreshButtonStyle}
        onMouseEnter={(e) => {
          e.currentTarget.style.color = 'var(--color-text-secondary)';
          e.currentTarget.style.borderColor = 'var(--color-text-muted)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = 'var(--color-text-muted)';
          e.currentTarget.style.borderColor = 'var(--color-border)';
        }}
        aria-label={t('splash.refreshIfStuck')}
        title={t('splash.refreshIfStuck')}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
          <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
        </svg>
        {t('action.refresh')}
      </button>

      {/* Animations */}
      <style>{splashKeyframes}</style>
    </div>
  );
}
