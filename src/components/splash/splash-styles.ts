// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { CSSProperties } from 'react';

// ── Types & Constants ──────────────────────────────────────────────

export type InitStage =
  | 'starting'
  | 'database'
  | 'embeddings'
  | 'context'
  | 'sources'
  | 'ready';

export const stageKeys: Record<InitStage, string> = {
  starting: 'splash.starting',
  database: 'splash.database',
  embeddings: 'splash.models',
  context: 'splash.context',
  sources: 'splash.sources',
  ready: 'splash.ready',
};

export const stageExplanations: Record<InitStage, string> = {
  starting: 'Initializing core systems',
  database: 'Loading your settings and history',
  embeddings: 'Checking AI provider availability',
  context: 'Scanning your project environment',
  sources: 'Preparing content pipelines',
  ready: 'All systems operational',
};

export const stageOrder: InitStage[] = [
  'starting', 'database', 'embeddings', 'context', 'sources', 'ready',
];

// ── CSS Keyframes ──────────────────────────────────────────────────

export const splashKeyframes = `
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  @keyframes pulse {
    0%, 100% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.02); opacity: 0.9; }
  }
`;

// ── Style Objects ──────────────────────────────────────────────────

export const containerStyle = (fadeOut: boolean): CSSProperties => ({
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
});

export const logoRingStyle: CSSProperties = {
  width: '10rem',
  height: '10rem',
  borderRadius: '50%',
  overflow: 'hidden',
  boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
  animation: 'pulse 2s ease-in-out infinite',
};

export const fallbackEmojiStyle: CSSProperties = {
  width: '100%',
  height: '100%',
  backgroundColor: 'var(--color-accent-gold)',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  fontSize: '3rem',
};

export const spinnerRingStyle: CSSProperties = {
  position: 'absolute',
  inset: '-8px',
  borderRadius: '50%',
  border: '2px solid transparent',
  borderTopColor: 'var(--color-accent-action)',
  animation: 'spin 1.5s linear infinite',
};

export const brandNameStyle: CSSProperties = {
  fontSize: '2.5rem',
  fontWeight: 600,
  color: 'var(--color-text-primary)',
  letterSpacing: '-0.025em',
  marginBottom: '0.5rem',
};

export const taglineStyle: CSSProperties = {
  fontSize: '1rem',
  color: 'var(--color-accent-action)',
  letterSpacing: '0.05em',
  marginBottom: '2.5rem',
  fontWeight: 500,
};

export const progressTrackStyle: CSSProperties = {
  width: '280px',
  height: '4px',
  backgroundColor: 'var(--color-bg-tertiary)',
  borderRadius: '2px',
  overflow: 'hidden',
  marginBottom: '1rem',
};

export const progressFillStyle = (progress: number): CSSProperties => ({
  height: '100%',
  width: `${progress}%`,
  backgroundColor: 'var(--color-accent-action)',
  borderRadius: '2px',
  transition: 'width 300ms ease-out',
});

export const miniSpinnerStyle: CSSProperties = {
  width: '16px',
  height: '16px',
  border: '2px solid var(--color-accent-action)',
  borderTopColor: 'transparent',
  borderRadius: '50%',
  animation: 'spin 0.8s linear infinite',
};

export const retryButtonStyle: CSSProperties = {
  marginTop: '1rem',
  padding: '0.5rem 1.5rem',
  background: 'var(--color-error)',
  color: '#fff',
  border: 'none',
  borderRadius: '0.375rem',
  cursor: 'pointer',
  fontSize: '0.8125rem',
};

export const versionStyle: CSSProperties = {
  position: 'absolute',
  bottom: '1.5rem',
  fontSize: '0.75rem',
  color: 'var(--color-text-muted)',
};

export const refreshButtonStyle: CSSProperties = {
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
};
