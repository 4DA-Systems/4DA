// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Vitest Test Setup
 *
 * This file runs before each test file.
 * Configure global test utilities here.
 */

import '@testing-library/jest-dom';

// Polyfill ResizeObserver for JSDOM (used by WebGL components)
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// Polyfill IntersectionObserver (used by virtualized lists).
if (typeof globalThis.IntersectionObserver === 'undefined') {
  globalThis.IntersectionObserver = class IntersectionObserver {
    readonly root = null;
    readonly rootMargin = '';
    readonly thresholds: ReadonlyArray<number> = [];
    observe() {}
    unobserve() {}
    disconnect() {}
    takeRecords() {
      return [];
    }
  } as unknown as typeof IntersectionObserver;
}

// Mock HTMLCanvasElement.getContext — jsdom does not implement any canvas
// contexts, so any fourda-component or briefing-atmosphere render
// spams "Error: Not implemented: HTMLCanvasElement.prototype.getContext"
// during every test file that imports one. Return a chainable no-op
// context object so the calling code thinks it succeeded.
// Ref: docs/ADVERSARIAL-AUDIT-2026-04-19.md P2 (frontend test noise).
if (typeof HTMLCanvasElement !== 'undefined') {
  const noopCtx = new Proxy(
    {},
    {
      get: (_target, prop) => {
        if (prop === 'canvas') return null;
        if (prop === 'getImageData') {
          return () => ({ data: new Uint8ClampedArray(4) });
        }
        if (prop === 'measureText') return () => ({ width: 0 });
        return () => noopCtx;
      },
    },
  );
  HTMLCanvasElement.prototype.getContext = (() => noopCtx) as unknown as typeof HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.toDataURL = () => 'data:,';
}

// Mock matchMedia — some components (Tailwind dark-mode queries, mobile
// breakpoint checks) call window.matchMedia. jsdom does not implement it.
if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

// Mock Tauri API for tests (since we can't access native APIs in jsdom)
vi.mock('@tauri-apps/api', () => ({
  invoke: vi.fn(),
  event: {
    listen: vi.fn(() => Promise.resolve(() => {})),
    emit: vi.fn(),
  },
}));

// Mock Tauri core (invoke) and event listener subpath imports
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
  Channel: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
  once: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock Tauri plugin opener
vi.mock('@tauri-apps/plugin-opener', () => ({
  open: vi.fn(),
  openUrl: vi.fn(),
}));

// Mock react-i18next — passthrough t() returns the key itself
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  I18nextProvider: ({ children }: { children: React.ReactNode }) => children,
}));

// Global test utilities
beforeEach(() => {
  // Reset mocks before each test
  vi.clearAllMocks();
});

// Console noise policy for tests.
//
// The previous policy only suppressed React `Warning:` prefixes. This
// expanded policy (ref docs/ADVERSARIAL-AUDIT-2026-04-19.md P2 frontend
// test noise) silences the three biggest noise classes while still
// surfacing genuine test-failure diagnostics:
//
//   1. jsdom "Not implemented" notices — we mock the surfaces we care
//      about above; the rest are expected and irrelevant to the test.
//   2. React "not wrapped in act(...)" warnings — our test style uses
//      userEvent + findBy queries, which already flush the microtask
//      queue. The act warnings are false positives in this codebase.
//   3. Intentional backend error logs from store slices that deliberately
//      console.debug() failures as part of resilience patterns.
//
// Genuine test failures, unhandled promise rejections, and unexpected
// throws still propagate — those don't go through console.error in our
// codebase.
const originalError = console.error;
const originalWarn = console.warn;

const SILENCED_PATTERNS = [
  /Not implemented: HTMLCanvasElement/,
  /Not implemented: navigation/,
  /Not implemented: HTMLMediaElement/,
  /not wrapped in act\(/,
  /inside a test was not wrapped in act/,
  /validateDOMNesting/, // Noisy when tests mount partial trees
];

beforeAll(() => {
  console.error = (...args) => {
    const msg = typeof args[0] === 'string' ? args[0] : '';
    if (SILENCED_PATTERNS.some((re) => re.test(msg))) return;
    if (msg.includes('Warning:')) return; // React dev warnings
    originalError(...args);
  };
  console.warn = (...args) => {
    const msg = typeof args[0] === 'string' ? args[0] : '';
    if (SILENCED_PATTERNS.some((re) => re.test(msg))) return;
    originalWarn(...args);
  };
});

afterAll(() => {
  console.error = originalError;
  console.warn = originalWarn;
});
