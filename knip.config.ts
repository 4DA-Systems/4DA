import type { KnipConfig } from 'knip';

const config: KnipConfig = {
  // `src/main.tsx` is reached automatically via index.html (knip's vite plugin),
  // so listing it here is redundant — knip flags it as such.
  entry: ['src/App.tsx'],
  project: ['src/**/*.{ts,tsx}'],
  ignore: [
    'src/test/**',
    // `site/` is a separate workspace with its OWN package.json (4da-site) and its
    // own deps. Scanning it from the root manifest reported @noble/ed25519 and
    // stripe as "unlisted dependencies" when they are correctly declared in
    // site/package.json — a false positive on every run.
    'site/**',
  ],
  ignoreDependencies: [
    '@types/*',
    // Tauri CLI — invoked via `pnpm tauri` script, not imported
    '@tauri-apps/cli',
    // Used as a CSS import (`@import "tailwindcss"` in src/App.css) and resolved
    // through @tailwindcss/vite, so knip's import graph cannot see it.
    'tailwindcss',
  ],
};

export default config;
