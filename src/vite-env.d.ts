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
    }
  }
}
