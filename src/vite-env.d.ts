/// <reference types="vite/client" />

import 'react';

declare global {
  const __APP_VERSION__: string;
}

// 4DA Web Component custom elements (fourda-*) — augment React's JSX namespace
declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'fourda-celebration-burst': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-status-orb': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-ambient-intelligence': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-score-fingerprint': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-decision-countdown': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-source-vitals': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-briefing-atmosphere': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-playbook-pathway': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-turing-fire': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-tetrahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-pentachoron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-icosahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-dodecahedron': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-simplex-unfold': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-compound-five-tetrahedra': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-momentum-field': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
      'fourda-logo-mark': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>;
    }
  }
}
