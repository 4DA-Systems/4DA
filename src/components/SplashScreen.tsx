import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import sunLogo from '../assets/sun-logo.jpg';

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

const stageMessages: Record<InitStage, string> = {
  starting: 'Starting up...',
  database: 'Initializing database...',
  embeddings: 'Loading AI models...',
  context: 'Preparing context engine...',
  sources: 'Connecting to sources...',
  ready: 'Ready!',
};

const stageOrder: InitStage[] = ['starting', 'database', 'embeddings', 'context', 'sources', 'ready'];

export function SplashScreen({ onComplete, minimumDisplayTime = 1500 }: SplashScreenProps) {
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

  // Check backend readiness
  useEffect(() => {
    let cancelled = false;

    const checkBackend = async () => {
      try {
        // Stage 1: Database
        setStage('database');
        await invoke('get_settings');
        if (cancelled) return;

        // Stage 2: Embeddings/AI
        setStage('embeddings');

        // Stage 3: Context engine
        setStage('context');
        try {
          await invoke('get_context_stats');
        } catch {
          // Context might not be set up yet, that's ok
        }
        if (cancelled) return;

        // Stage 4: Sources
        setStage('sources');
        try {
          await invoke('get_sources');
        } catch {
          // Sources might fail, that's ok for startup
        }
        if (cancelled) return;

        // Stage 5: Ready
        setStage('ready');
        setBackendReady(true);

      } catch (e) {
        console.error('[SplashScreen] Backend check failed:', e);
        setError(String(e));
        // Still mark as ready after error so app can show error state
        setBackendReady(true);
      }
    };

    checkBackend();

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
      aria-label={error ? 'Error during startup' : stageMessages[stage]}
      aria-busy={stage !== 'ready'}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 50,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        backgroundColor: 'var(--color-bg-primary)',
        transition: 'opacity 300ms',
        opacity: fadeOut ? 0 : 1,
      }}
    >
      {/* Sun Logo with pulse animation */}
      <div style={{ position: 'relative', marginBottom: '2rem' }}>
        <div style={{
          width: '10rem',
          height: '10rem',
          borderRadius: '50%',
          overflow: 'hidden',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
          animation: 'pulse 2s ease-in-out infinite',
        }}>
          {!imageError ? (
            <img
              src={sunLogo}
              alt="4DA"
              style={{ width: '100%', height: '100%', objectFit: 'cover' }}
              onError={() => setImageError(true)}
            />
          ) : (
            <div style={{
              width: '100%',
              height: '100%',
              backgroundColor: 'var(--color-accent-gold)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '3rem',
            }}>
              ☀️
            </div>
          )}
        </div>
        {/* Animated glow ring */}
        <div style={{
          position: 'absolute',
          inset: '-8px',
          borderRadius: '50%',
          border: '2px solid transparent',
          borderTopColor: '#F97316',
          animation: 'spin 1.5s linear infinite',
        }} />
      </div>

      {/* Brand Name */}
      <h1 style={{
        fontSize: '2.5rem',
        fontWeight: 600,
        color: '#FFFFFF',
        letterSpacing: '-0.025em',
        marginBottom: '0.5rem',
      }}>
        4DA
      </h1>

      {/* Tagline */}
      <p style={{
        fontSize: '1rem',
        color: '#F97316',
        letterSpacing: '0.05em',
        marginBottom: '2.5rem',
        fontWeight: 500,
      }}>
        All signal. No feed.
      </p>

      {/* Progress bar */}
      <div style={{
        width: '280px',
        height: '4px',
        backgroundColor: 'var(--color-bg-tertiary)',
        borderRadius: '2px',
        overflow: 'hidden',
        marginBottom: '1rem',
      }}>
        <div
          role="progressbar"
          aria-valuenow={Math.round(progress)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={`Startup progress: ${Math.round(progress)}%`}
          style={{
            height: '100%',
            width: `${progress}%`,
            backgroundColor: '#F97316',
            borderRadius: '2px',
            transition: 'width 300ms ease-out',
          }}
        />
      </div>

      {/* Status message */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '0.75rem',
        minHeight: '24px',
      }}>
        {stage !== 'ready' && !error && (
          <div style={{
            width: '16px',
            height: '16px',
            border: '2px solid #F97316',
            borderTopColor: 'transparent',
            borderRadius: '50%',
            animation: 'spin 0.8s linear infinite',
          }} />
        )}
        {stage === 'ready' && !error && (
          <span style={{ color: 'var(--color-success)', fontSize: '1rem' }}>✓</span>
        )}
        {error && (
          <span style={{ color: 'var(--color-error)', fontSize: '1rem' }}>⚠</span>
        )}
        <span style={{
          fontSize: '0.875rem',
          color: error ? 'var(--color-error)' : stage === 'ready' ? 'var(--color-success)' : '#9CA3AF',
          transition: 'color 300ms',
        }}>
          {error ? 'Error during startup' : stageMessages[stage]}
        </span>
      </div>

      {/* Stage indicators */}
      <div style={{
        display: 'flex',
        gap: '0.5rem',
        marginTop: '1.5rem',
      }}>
        {stageOrder.slice(0, -1).map((s, i) => (
          <div
            key={s}
            style={{
              width: '8px',
              height: '8px',
              borderRadius: '50%',
              backgroundColor: i <= currentStageIndex ? '#F97316' : 'var(--color-border)',
              transition: 'background-color 300ms',
            }}
          />
        ))}
      </div>

      {/* Version */}
      <p style={{
        position: 'absolute',
        bottom: '1.5rem',
        fontSize: '0.75rem',
        color: '#4B5563',
      }}>
        Version 1.0.0
      </p>

      {/* Subtle refresh button - top right corner */}
      <button
        onClick={() => window.location.reload()}
        style={{
          position: 'absolute',
          top: '1rem',
          right: '1rem',
          padding: '0.5rem 0.75rem',
          fontSize: '0.75rem',
          color: 'var(--color-text-muted)',
          backgroundColor: 'transparent',
          border: '1px solid var(--color-border)',
          borderRadius: '6px',
          cursor: 'pointer',
          transition: 'all 200ms',
          display: 'flex',
          alignItems: 'center',
          gap: '0.375rem',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.color = '#9CA3AF';
          e.currentTarget.style.borderColor = '#4B5563';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.color = 'var(--color-text-muted)';
          e.currentTarget.style.borderColor = 'var(--color-border)';
        }}
        title="Refresh if stuck"
      >
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
        </svg>
        Refresh
      </button>

      {/* Animations */}
      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        @keyframes pulse {
          0%, 100% { transform: scale(1); opacity: 1; }
          50% { transform: scale(1.02); opacity: 0.9; }
        }
      `}</style>
    </div>
  );
}
