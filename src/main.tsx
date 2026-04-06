// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import React from 'react';
import ReactDOM from 'react-dom/client';
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import App from './App';

// Signal Rust that the frontend JS loaded BEFORE React mounts.
// This fires ~300-500ms earlier than SplashScreen's useEffect,
// allowing the hidden window to show immediately with the splash
// animation instead of waiting for the full component tree.
try {
  const { emit } = await import('@tauri-apps/api/event');
  const result = emit('frontend-ready');
  if (result && typeof result.catch === 'function') {
    result.catch(() => { /* ignore in browser mode */ });
  }
} catch {
  // Non-Tauri environment (tests, browser) — silently ignore
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
