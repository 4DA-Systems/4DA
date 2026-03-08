/**
 * Vitest Test Setup
 *
 * This file runs before each test file.
 * Configure global test utilities here.
 */

import '@testing-library/jest-dom';

// Polyfill ResizeObserver for JSDOM (used by GAME WebGL components)
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
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

// Suppress console errors in tests unless explicitly testing error handling
const originalError = console.error;
beforeAll(() => {
  console.error = (...args) => {
    // Allow React testing library warnings through
    if (typeof args[0] === 'string' && args[0].includes('Warning:')) {
      return;
    }
    originalError(...args);
  };
});

afterAll(() => {
  console.error = originalError;
});
