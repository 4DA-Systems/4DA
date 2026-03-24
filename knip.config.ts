import type { KnipConfig } from 'knip';

const config: KnipConfig = {
  entry: ['src/App.tsx'],
  project: ['src/**/*.{ts,tsx}'],
  ignore: [
    'src/test/**',
  ],
  ignoreDependencies: [
    '@types/*',
  ],
};

export default config;
