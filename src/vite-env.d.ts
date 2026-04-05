/// <reference types="vite/client" />

import 'react';

declare global {
  const __APP_VERSION__: string;
}

// GAME Web Component custom elements — augment React's JSX namespace
declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'game-celebration-burst': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-scan-ring': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-status-orb': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-boot-ring': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-engagement-bars': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-achievement-progress': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-ambient-intelligence': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-score-fingerprint': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-decision-countdown': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-signal-waveform': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-knowledge-depth': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-source-vitals': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-briefing-atmosphere': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-playbook-pathway': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-radar-field': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-turing-fire': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-intelligence-banner': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-tetrahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-pentachoron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-icosahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-dodecahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-simplex-unfold': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-compound-five-tetrahedra': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-logo-mark': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
    }
  }
}
