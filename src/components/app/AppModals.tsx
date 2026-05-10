// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

import { lazy, Suspense } from 'react';
import { ViewErrorBoundary } from '../ViewErrorBoundary';
import { ZoomIndicator } from '../ZoomIndicator';
import { FeedbackMilestone } from '../FeedbackMilestone';
import type { Toast } from '../../hooks/use-toasts';
import { ToastContainer } from '../Toast';

// Lazy-loaded non-critical views and overlays
const MilestoneOverlay = lazy(() => import('../MilestoneOverlay').then(m => ({ default: m.MilestoneOverlay })));
const GuidedHighlights = lazy(() => import('../GuidedHighlights').then(m => ({ default: m.GuidedHighlights })));
const SettingsModal = lazy(() => import('../SettingsModal').then(m => ({ default: m.SettingsModal })));
const KeyboardShortcutsModal = lazy(() => import('../KeyboardShortcutsModal').then(m => ({ default: m.KeyboardShortcutsModal })));
const FrameworkPage = lazy(() => import('../FrameworkPage').then(m => ({ default: m.FrameworkPage })));
const ComparisonPage = lazy(() => import('../ComparisonPage').then(m => ({ default: m.ComparisonPage })));

interface AppModalsProps {
  toasts: Toast[];
  removeToast: (id: number) => void;
  zoom: number;
  showZoomIndicator: boolean;
  feedbackCount: number;
  showKeyboardHelp: boolean;
  setShowKeyboardHelp: (v: boolean) => void;
  showFramework: boolean;
  setShowFramework: (v: boolean) => void;
  showComparison: boolean;
  setShowComparison: (v: boolean) => void;
  showSettings: boolean;
  setShowSettings: (v: boolean) => void;
}

/**
 * All overlay/modal components rendered at the bottom of the App JSX tree.
 * Extracted from App.tsx to reduce its line count.
 */
export function AppModals({
  toasts, removeToast,
  zoom, showZoomIndicator,
  feedbackCount,
  showKeyboardHelp, setShowKeyboardHelp,
  showFramework, setShowFramework,
  showComparison, setShowComparison,
  showSettings, setShowSettings,
}: AppModalsProps) {
  return (
    <>
      {/* Toast Notifications */}
      <ToastContainer toasts={toasts} onDismiss={removeToast} />
      <ZoomIndicator zoom={zoom} visible={showZoomIndicator} />
      <FeedbackMilestone count={feedbackCount} />
      <Suspense fallback={null}><MilestoneOverlay /></Suspense>

      {/* Guided Highlights — one-time feature discovery overlay (self-dismisses via localStorage) */}
      <Suspense fallback={null}><GuidedHighlights /></Suspense>

      {/* Keyboard Shortcuts Help Modal */}
      {showKeyboardHelp && (
        <Suspense fallback={null}>
          <ViewErrorBoundary viewName="Keyboard Shortcuts">
            <KeyboardShortcutsModal onClose={() => setShowKeyboardHelp(false)} />
          </ViewErrorBoundary>
        </Suspense>
      )}

      {/* Framework Page — philosophy publication */}
      {showFramework && (
        <Suspense fallback={null}>
          <ViewErrorBoundary viewName="Framework">
            <FrameworkPage onClose={() => setShowFramework(false)} />
          </ViewErrorBoundary>
        </Suspense>
      )}

      {/* Comparison Page — competitive positioning */}
      {showComparison && (
        <Suspense fallback={null}>
          <ViewErrorBoundary viewName="Comparison">
            <ComparisonPage onClose={() => setShowComparison(false)} />
          </ViewErrorBoundary>
        </Suspense>
      )}

      {/* Settings Modal - now self-sufficient via Zustand store */}
      {showSettings && (
        <Suspense fallback={null}>
          <ViewErrorBoundary viewName="Settings" onReset={() => setShowSettings(false)}>
            <SettingsModal
              onClose={() => setShowSettings(false)}
            />
          </ViewErrorBoundary>
        </Suspense>
      )}
    </>
  );
}
