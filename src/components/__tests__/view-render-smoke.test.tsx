// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * View Render Smoke Test
 *
 * For every view that the navigation router can produce, mount it in
 * isolation with a minimal store and verify it doesn't throw during
 * initial render. Catches the "view compiles but crashes on mount"
 * class of bug that the TIER_VIEWS fix alone wouldn't have caught.
 *
 * NOTE: Uses dynamic imports so any per-view crash is caught and
 * reported as a test failure rather than a module-load failure.
 */

import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { Suspense } from 'react';

// Stub IPC so views that load data on mount don't hit a missing Tauri runtime
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(null)),
}));

// Stub the fourda-component hook so WebGPU isn't required
vi.mock('../../hooks/use-fourda-component', () => ({
  useFourdaComponent: () => ({
    containerRef: { current: null },
    elementRef: { current: null },
  }),
}));

// Every view that ViewRouter can render
const VIEWS = [
  { id: 'preemption', module: () => import('../preemption/PreemptionView') },
  { id: 'blindspots', module: () => import('../blindspots/BlindSpotsView') },
];

describe('View render smoke', () => {
  for (const view of VIEWS) {
    it(`view "${view.id}" mounts without throwing`, async () => {
      const Component = (await view.module()).default;
      expect(Component).toBeDefined();
      expect(() => {
        render(
          <Suspense fallback={null}>
            <Component />
          </Suspense>,
        );
      }).not.toThrow();
    });
  }
});
