import { useState, useEffect, useCallback, useRef } from 'react';

const ZOOM_MIN = 0.75;
const ZOOM_MAX = 1.50;
const ZOOM_STEP = 0.10;
const STORAGE_KEY = '4da-ui-zoom';
const INDICATOR_MS = 1500;

function clampZoom(value: number): number {
  return Math.round(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, value)) * 100) / 100;
}

function readStoredZoom(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw !== null) {
      const parsed = parseFloat(raw);
      if (!isNaN(parsed)) return clampZoom(parsed);
    }
  } catch {
    // localStorage unavailable
  }
  return 1.0;
}

export function useUiZoom() {
  const [zoom, setZoom] = useState(readStoredZoom);
  const [showIndicator, setShowIndicator] = useState(false);
  const indicatorTimeout = useRef<ReturnType<typeof setTimeout>>(undefined);

  const flashIndicator = useCallback(() => {
    setShowIndicator(true);
    if (indicatorTimeout.current) clearTimeout(indicatorTimeout.current);
    indicatorTimeout.current = setTimeout(() => setShowIndicator(false), INDICATOR_MS);
  }, []);

  // Apply stored zoom on mount
  useEffect(() => {
    document.documentElement.style.zoom = String(zoom);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Wheel handler
  const handleWheel = useCallback((e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    setZoom(prev => {
      const next = clampZoom(prev + (e.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
      document.documentElement.style.zoom = String(next);
      try {
        localStorage.setItem(STORAGE_KEY, String(next));
      } catch {
        // localStorage unavailable
      }
      return next;
    });
    flashIndicator();
  }, [flashIndicator]);

  // Keyboard handler — uses setZoom(prev =>) to avoid stale closure
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!e.ctrlKey && !e.metaKey) return;

    let delta: number | null = null;
    let reset = false;

    if (e.code === 'Equal' || e.code === 'NumpadAdd') delta = ZOOM_STEP;
    else if (e.code === 'Minus' || e.code === 'NumpadSubtract') delta = -ZOOM_STEP;
    else if (e.code === 'Digit0' || e.code === 'Numpad0') reset = true;

    if (delta === null && !reset) return;
    e.preventDefault();

    setZoom(prev => {
      const next = clampZoom(reset ? 1.0 : prev + (delta ?? 0));
      document.documentElement.style.zoom = String(next);
      try { localStorage.setItem(STORAGE_KEY, String(next)); } catch { /* */ }
      return next;
    });
    flashIndicator();
  }, [flashIndicator]);

  useEffect(() => {
    window.addEventListener('wheel', handleWheel, { passive: false });
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('wheel', handleWheel);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleWheel, handleKeyDown]);

  // Cleanup indicator timeout on unmount
  useEffect(() => {
    return () => {
      if (indicatorTimeout.current) clearTimeout(indicatorTimeout.current);
    };
  }, []);

  return { zoom, showIndicator };
}
