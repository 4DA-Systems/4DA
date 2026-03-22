import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import sunLogo from '../assets/sun-logo.webp';
import { translateError } from '../utils/error-messages';
import { TuringPattern } from './TuringPattern';
import { registerGameComponent } from '../lib/game-components';

/**
 * V2 Splash Screen — Turing pattern background with gold-on-dark palette.
 *
 * Falls back to the original sun logo if WebGPU is unavailable.
 * The pattern gets 6 seconds minimum to evolve before the app loads.
 * GPU resources are destroyed immediately on transition.
 */

interface SplashScreenProps {
  onComplete: () => void;
  minimumDisplayTime?: number;
}

type InitStage =
  | 'starting'
  | 'database'
  | 'embeddings'
  | 'context'
  | 'sources'
  | 'ready';

const stageKeys: Record<InitStage, string> = {
  starting: 'splash.starting',
  database: 'splash.database',
  embeddings: 'splash.models',
  context: 'splash.context',
  sources: 'splash.sources',
  ready: 'splash.ready',
};

const stageOrder: InitStage[] = ['starting', 'database', 'embeddings', 'context', 'sources', 'ready'];

export function SplashScreenV2({ onComplete, minimumDisplayTime = 6000 }: SplashScreenProps) {
  const { t } = useTranslation();
  const [fadeOut, setFadeOut] = useState(false);
  const [stage, setStage] = useState<InitStage>('starting');
  const [backendReady, setBackendReady] = useState(false);
  const [minTimeElapsed, setMinTimeElapsed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showTuring, setShowTuring] = useState(true);
  const [hasWebGPU, setHasWebGPU] = useState(true); // optimistic

  useEffect(() => { registerGameComponent('game-pentachoron'); }, []);

  // Minimum display time — gives the pattern time to emerge
  useEffect(() => {
    const timer = setTimeout(() => setMinTimeElapsed(true), minimumDisplayTime);
    return () => clearTimeout(timer);
  }, [minimumDisplayTime]);

  // Backend initialization
  useEffect(() => {
    let cancelled = false;
    const checkBackend = async () => {
      try {
        setStage('database');
        await cmd('get_settings');
        if (cancelled) return;

        setStage('embeddings');
        await Promise.allSettled([
          cmd('get_context_stats'),
          cmd('get_sources'),
        ]);
        if (cancelled) return;

        setStage('ready');
        setBackendReady(true);
      } catch (e) {
        console.error('[SplashScreen] Backend check failed:', e);
        const isBrowser = !('__TAURI_INTERNALS__' in window);
        if (isBrowser) {
          // In real browsers, redirect to Signal Terminal (skip in test/JSDOM)
          if (!import.meta.env.VITEST) {
            const terminalPort = import.meta.env.DEV ? 4445 : 4444;
            window.location.href = `http://localhost:${terminalPort}/`;
            return;
          }
          setError('Desktop app required \u2014 open through Tauri window');
        } else {
          setError(translateError(e));
        }
      }
    };
    checkBackend();
    return () => { cancelled = true; };
  }, []);

  // Transition when ready
  useEffect(() => {
    if (backendReady && minTimeElapsed) {
      setFadeOut(true);
      // Kill GPU resources immediately
      setShowTuring(false);
      setTimeout(onComplete, 700);
    }
  }, [backendReady, minTimeElapsed, onComplete]);

  const currentStageIndex = stageOrder.indexOf(stage);
  const progress = ((currentStageIndex + 1) / stageOrder.length) * 100;

  return (
    <div
      role="status"
      aria-label={error ? t('splash.error') : t(stageKeys[stage])}
      aria-busy={stage !== 'ready'}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 50,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: '#050505',
        transition: 'opacity 700ms ease-out',
        opacity: fadeOut ? 0 : 1,
      }}
    >
      {/* ── Turing pattern background ── */}
      {showTuring && hasWebGPU && (
        <div style={{
          position: 'absolute',
          inset: 0,
          transition: 'opacity 1000ms ease-out',
          opacity: fadeOut ? 0 : 1,
        }}>
          <TuringPattern
            feed={0.037}
            kill={0.06}
            gridSize={384}
            stepsPerFrame={8}
            onFallback={() => setHasWebGPU(false)}
          />
        </div>
      )}

      {/* ── Content ── */}
      <div style={{
        position: 'relative',
        zIndex: 1,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
      }}>
        {/* Brand mark */}
        <div style={{ position: 'relative', marginBottom: '2rem' }}>
          <div style={{
            width: '8rem',
            height: '8rem',
            borderRadius: '50%',
            overflow: 'hidden',
            border: '2px solid rgba(212, 175, 55, 0.25)',
            boxShadow: '0 0 80px rgba(212, 175, 55, 0.1), inset 0 0 40px rgba(0, 0, 0, 0.6)',
            animation: 'splash-breathe 4s ease-in-out infinite',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            backgroundColor: 'rgba(5, 5, 5, 0.5)',
            backdropFilter: 'blur(8px)',
          }}>
            {hasWebGPU ? (
              <span style={{
                fontSize: '2.5rem',
                fontWeight: 700,
                color: '#D4AF37',
                letterSpacing: '-0.05em',
                textShadow: '0 0 30px rgba(212, 175, 55, 0.3)',
              }}>
                4
              </span>
            ) : (
              <img
                src={sunLogo}
                alt="4DA"
                style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                onError={(e) => {
                  ((e.target) as globalThis.HTMLImageElement).style.display = 'none';
                }}
              />
            )}
          </div>
          {/* Orbital ring */}
          <div style={{
            position: 'absolute',
            inset: '-6px',
            borderRadius: '50%',
            border: '1.5px solid transparent',
            borderTopColor: 'rgba(212, 175, 55, 0.5)',
            animation: 'splash-spin 3s linear infinite',
          }} />
        </div>

        {/* Brand name */}
        <h1 style={{
          fontSize: '2rem',
          fontWeight: 600,
          color: '#FFFFFF',
          letterSpacing: '0.1em',
          marginBottom: '0.375rem',
          textShadow: hasWebGPU ? '0 2px 20px rgba(0, 0, 0, 0.8)' : 'none',
        }}>
          4DA
        </h1>

        {/* Pentachoron accent — 4D identity mark */}
        <div style={{
          width: 56,
          height: 56,
          marginBottom: '0.75rem',
          opacity: 0.2,
          borderRadius: 8,
          overflow: 'hidden',
        }}>
          <game-pentachoron style={{ width: '56px', height: '56px', display: 'block' }} />
        </div>

        {/* Tagline */}
        <p style={{
          fontSize: '0.75rem',
          color: '#D4AF37',
          letterSpacing: '0.2em',
          marginBottom: '2.5rem',
          fontWeight: 400,
          textTransform: 'uppercase',
          opacity: 0.7,
          textShadow: hasWebGPU ? '0 1px 10px rgba(0, 0, 0, 0.8)' : 'none',
        }}>
          {t('app.tagline')}
        </p>

        {/* Progress bar */}
        <div style={{
          width: '180px',
          height: '2px',
          backgroundColor: 'rgba(255, 255, 255, 0.06)',
          borderRadius: '1px',
          overflow: 'hidden',
          marginBottom: '1rem',
        }}>
          <div
            role="progressbar"
            aria-valuenow={Math.round(progress)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={t('splash.progress', { percent: Math.round(progress) })}
            style={{
              height: '100%',
              width: `${progress}%`,
              background: 'linear-gradient(90deg, #D4AF37, #F5D680)',
              borderRadius: '1px',
              transition: 'width 400ms ease-out',
            }}
          />
        </div>

        {/* Status */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '0.625rem',
          minHeight: '20px',
        }}>
          {stage !== 'ready' && !error && (
            <div style={{
              width: '12px',
              height: '12px',
              border: '1.5px solid rgba(212, 175, 55, 0.6)',
              borderTopColor: 'transparent',
              borderRadius: '50%',
              animation: 'splash-spin 0.8s linear infinite',
            }} />
          )}
          {stage === 'ready' && !error && (
            <span style={{ color: '#22C55E', fontSize: '0.8125rem' }}>&#10003;</span>
          )}
          {error && (
            <span style={{ color: '#EF4444', fontSize: '0.8125rem' }}>&#9888;</span>
          )}
          <span style={{
            fontSize: '0.6875rem',
            color: error ? '#EF4444' : stage === 'ready' ? '#22C55E' : 'rgba(255, 255, 255, 0.3)',
            letterSpacing: '0.05em',
            transition: 'color 300ms',
            textShadow: hasWebGPU ? '0 1px 8px rgba(0, 0, 0, 0.9)' : 'none',
          }}>
            {error || t(stageKeys[stage])}
          </span>
        </div>
        {error && (
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '1rem',
              padding: '0.5rem 1.5rem',
              background: '#EF4444',
              color: '#fff',
              border: 'none',
              borderRadius: '0.375rem',
              cursor: 'pointer',
              fontSize: '0.8125rem',
            }}
          >
            {t('action.retry')}
          </button>
        )}

        {/* Stage dots */}
        <div style={{
          display: 'flex',
          gap: '6px',
          marginTop: '1.25rem',
        }}>
          {stageOrder.slice(0, -1).map((s, i) => (
            <div
              key={s}
              style={{
                width: '4px',
                height: '4px',
                borderRadius: '50%',
                backgroundColor: i <= currentStageIndex
                  ? '#D4AF37'
                  : 'rgba(255, 255, 255, 0.08)',
                transition: 'background-color 300ms',
              }}
            />
          ))}
        </div>
      </div>

      {/* Version */}
      <p style={{
        position: 'absolute',
        bottom: '1.5rem',
        fontSize: '0.6875rem',
        color: 'rgba(255, 255, 255, 0.12)',
        letterSpacing: '0.05em',
        zIndex: 1,
      }}>
        {t('splash.version', { version: __APP_VERSION__ })}
      </p>

      {/* Refresh */}
      <button
        onClick={() => window.location.reload()}
        style={{
          position: 'absolute',
          top: '1rem',
          right: '1rem',
          padding: '0.5rem 0.75rem',
          fontSize: '0.6875rem',
          color: 'rgba(255, 255, 255, 0.15)',
          backgroundColor: 'transparent',
          border: '1px solid rgba(255, 255, 255, 0.06)',
          borderRadius: '6px',
          cursor: 'pointer',
          transition: 'all 200ms',
          display: 'flex',
          alignItems: 'center',
          gap: '0.375rem',
          zIndex: 1,
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.color = 'rgba(255, 255, 255, 0.4)';
          e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.15)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = 'rgba(255, 255, 255, 0.15)';
          e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.06)';
        }}
        aria-label={t('splash.refreshIfStuck')}
        title={t('splash.refreshIfStuck')}
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
          <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
        </svg>
        {t('action.refresh')}
      </button>

      <style>{`
        @keyframes splash-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        @keyframes splash-breathe {
          0%, 100% { transform: scale(1); }
          50% { transform: scale(1.02); }
        }
      `}</style>
    </div>
  );
}
