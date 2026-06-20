// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Idle deferral helper for non-paint-critical startup work.
//
// Measured 2026-06-20 (live, warm dev session): when the React tree mounts it
// fires ~10+ Tauri commands at once. They serialize over the IPC bridge while
// the webview main thread is still parsing/executing the JS bundle, so unrelated
// commands clustered at ~230ms even though their real compute was ~30-70ms — the
// extra time was pure queue depth. The worst single offender, the
// `prune_personalization_cache` maintenance command (576-902ms), was firing
// fire-and-forget directly on the mount path.
//
// `runWhenIdle` moves non-critical work off that stampede: it runs after the
// browser reports idle (requestIdleCallback, available in WebView2/Chromium),
// falling back to a short timeout on engines without it (notably older WKWebView
// on macOS). Returns a cancel fn suitable for a React effect cleanup.

/**
 * Run `fn` once the main thread is idle (or after a short fallback delay on
 * engines without requestIdleCallback). Returns a cancel function — pass it
 * straight through as a React `useEffect` cleanup to avoid running after unmount.
 *
 * requestIdleCallback is typed on `Window` by lib.dom but is NOT implemented by
 * every webview engine at runtime (older WKWebView on macOS), so we still guard
 * with a typeof check and fall back to a timeout.
 *
 * @param fallbackDelayMs delay used when requestIdleCallback is unavailable.
 *        Kept small (post-first-paint) so deferred work still lands promptly.
 */
export function runWhenIdle(fn: () => void, fallbackDelayMs = 350): () => void {
  if (typeof window.requestIdleCallback === 'function') {
    // timeout guarantees the work runs even if the thread never goes fully idle
    const handle = window.requestIdleCallback(fn, { timeout: 2000 });
    return () => {
      if (typeof window.cancelIdleCallback === 'function') window.cancelIdleCallback(handle);
    };
  }

  const id = setTimeout(fn, fallbackDelayMs);
  return () => clearTimeout(id);
}
