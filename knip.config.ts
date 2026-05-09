import type { KnipConfig } from 'knip';

const config: KnipConfig = {
  entry: ['src/main.tsx', 'src/App.tsx'],
  project: ['src/**/*.{ts,tsx}'],
  ignore: [
    'src/test/**',
    'src/generated/**',
  ],
  ignoreDependencies: [
    '@types/*',
    // Tauri CLI — invoked via `pnpm tauri` script, not imported
    '@tauri-apps/cli',
  ],
};

export default config;
