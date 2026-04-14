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
      'game-status-orb': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-ambient-intelligence': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-score-fingerprint': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-decision-countdown': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-source-vitals': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-briefing-atmosphere': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-playbook-pathway': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-turing-fire': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-tetrahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-pentachoron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-icosahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-dodecahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-simplex-unfold': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-compound-five-tetrahedra': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-momentum-field': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'game-logo-mark': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
    }
  }
}
