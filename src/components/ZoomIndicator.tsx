interface ZoomIndicatorProps {
  zoom: number;
  visible: boolean;
}

export function ZoomIndicator({ zoom, visible }: ZoomIndicatorProps) {
  if (!visible) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-bg-secondary/90 backdrop-blur-sm border border-border text-xs font-mono text-text-secondary px-3 py-1.5 rounded-lg pointer-events-none animate-zoom-fade"
    >
      {Math.round(zoom * 100)}%
    </div>
  );
}
