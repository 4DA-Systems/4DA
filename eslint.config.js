import js from '@eslint/js';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import i18next from 'eslint-plugin-i18next';

export default [
  js.configs.recommended,
  // jsx-a11y recommended (flat config) — provides plugin + baseline rules
  jsxA11y.flatConfigs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: {
          jsx: true,
        },
        // Type-checked linting — enables no-floating-promises, no-misused-promises
        project: './tsconfig.json',
        tsconfigRootDir: import.meta.dirname,
      },
      globals: {
        window: 'readonly',
        document: 'readonly',
        console: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        setInterval: 'readonly',
        clearInterval: 'readonly',
        fetch: 'readonly',
        URL: 'readonly',
        Blob: 'readonly',
        HTMLElement: 'readonly',
        Event: 'readonly',
        MouseEvent: 'readonly',
        KeyboardEvent: 'readonly',
        HTMLInputElement: 'readonly',
        HTMLTextAreaElement: 'readonly',
        HTMLSelectElement: 'readonly',
        HTMLButtonElement: 'readonly',
        HTMLFormElement: 'readonly',
        HTMLDivElement: 'readonly',
        HTMLCanvasElement: 'readonly',
        WebGL2RenderingContext: 'readonly',
        WebGLProgram: 'readonly',
        React: 'readonly',
        requestAnimationFrame: 'readonly',
        cancelAnimationFrame: 'readonly',
        NodeJS: 'readonly',
        navigator: 'readonly',
        localStorage: 'readonly',
        crypto: 'readonly',
        btoa: 'readonly',
        atob: 'readonly',
        TextEncoder: 'readonly',
        TextDecoder: 'readonly',
        Uint8Array: 'readonly',
        ArrayBuffer: 'readonly',
        __APP_VERSION__: 'readonly',
        customElements: 'readonly',
        performance: 'readonly',
        ResizeObserver: 'readonly',
        GPUShaderStage: 'readonly',
        GPUBufferUsage: 'readonly',
      },
    },
    plugins: {
      '@typescript-eslint': tsPlugin,
      'react': reactPlugin,
      'react-hooks': reactHooksPlugin,
      'i18next': i18next,
    },
    rules: {
      // TypeScript rules
      '@typescript-eslint/no-unused-vars': ['warn', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      }],
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/explicit-module-boundary-types': 'off',

      // TypeScript strict rules (type-checked)
      '@typescript-eslint/no-floating-promises': 'warn',
      '@typescript-eslint/no-misused-promises': 'warn',
      // strict-boolean-expressions: 501 violations project-wide, aspirational, not
      // being followed. Turned off rather than leaving as perpetual noise. If we
      // decide to enforce explicit boolean coercion later, reintroduce with a
      // scheduled cleanup — not a passive warning.
      '@typescript-eslint/strict-boolean-expressions': 'off',
      '@typescript-eslint/consistent-type-imports': 'warn',
      '@typescript-eslint/no-unnecessary-type-assertion': 'warn',

      // React rules
      'react/react-in-jsx-scope': 'off', // Not needed in React 18+
      'react/prop-types': 'off', // Using TypeScript for prop types
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',

      // jsx-a11y overrides — desktop app patterns get warnings, not errors
      'jsx-a11y/alt-text': 'error',
      'jsx-a11y/aria-props': 'error',
      'jsx-a11y/aria-role': 'error',
      'jsx-a11y/click-events-have-key-events': 'warn',
      'jsx-a11y/no-static-element-interactions': 'warn',
      'jsx-a11y/label-has-associated-control': 'warn',
      'jsx-a11y/anchor-is-valid': 'warn',

      // i18next — flag hardcoded strings in JSX
      'i18next/no-literal-string': ['warn', {
        markupOnly: true,
        ignoreAttribute: [
          'className', 'style', 'type', 'name', 'id',
          'data-testid', 'key', 'role', 'htmlFor', 'placeholder',
        ],
        ignoreCallee: [
          'console.log', 'console.warn', 'console.error',
          'reportError', 'reportWarning',
        ],
      }],

      // Prevent raw invoke() — use cmd() from lib/commands.ts
      'no-restricted-imports': ['error', {
        paths: [{
          name: '@tauri-apps/api/core',
          importNames: ['invoke'],
          message: 'Use cmd() from lib/commands.ts instead of raw invoke().',
        }],
      }],

      // General rules
      'no-console': 'off', // Allow console for debugging
      'no-unused-vars': 'off', // TypeScript handles this
      'prefer-const': 'warn',
      'no-var': 'error',

      // Code style — OFF. No Prettier config exists; these rules were generating
      // ~5800 quote-style warnings on JSX attributes alone. Style is not the job
      // of type-checked lint. Add Prettier later if we want automated formatting.
      'semi': 'off',
      'quotes': 'off',
      'comma-dangle': 'off',
    },
    settings: {
      react: {
        version: 'detect',
      },
    },
  },
  // Test files configuration with Vitest globals
  {
    files: ['src/**/*.test.{ts,tsx}', 'src/**/*.spec.{ts,tsx}', 'src/test/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-imports': 'off', // Tests mock invoke() directly
      // Relax i18next in tests — test strings are not user-facing
      'i18next/no-literal-string': 'off',
      // Relax type-checked rules in tests — test patterns often use loose promises
      '@typescript-eslint/no-floating-promises': 'off',
      '@typescript-eslint/no-misused-promises': 'off',
      '@typescript-eslint/strict-boolean-expressions': 'off',
    },
    languageOptions: {
      globals: {
        // Vitest globals
        vi: 'readonly',
        describe: 'readonly',
        it: 'readonly',
        test: 'readonly',
        expect: 'readonly',
        beforeEach: 'readonly',
        afterEach: 'readonly',
        beforeAll: 'readonly',
        afterAll: 'readonly',
      },
    },
  },
  // Allow lib/commands.ts to import invoke (it's the single source of truth)
  {
    files: ['src/lib/commands.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
  {
    ignores: [
      'node_modules/**',
      'dist/**',
      'src-tauri/**',
      '*.config.js',
      '*.config.ts',
      'scripts/**',
      'src/lib/fourda-components/**',
    ],
  },
];
