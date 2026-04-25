// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { FeedHealth } from '../../lib/commands';

interface FeedHealthDotProps {
  health: FeedHealth | undefined;
  onReset?: () => void;
}

function relativeTime(isoStr: string | null): string {
  if (!isoStr) return '';
  const diff = Date.now() - new Date(isoStr + 'Z').getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

export function FeedHealthDot({ health, onReset }: FeedHealthDotProps) {
  if (!health || (health.consecutive_failures === 0 && !health.circuit_open)) {
    return null;
  }

  if (health.circuit_open) {
    return (
      <span className="inline-flex items-center gap-1 ms-1.5">
        <span className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" title={health.last_error ?? 'Circuit open'} />
        <span className="text-[10px] text-red-400/70">
          paused
        </span>
        {onReset && (
          <button
            onClick={(e) => { e.stopPropagation(); onReset(); }}
            className="text-[10px] text-text-muted hover:text-white underline"
          >
            reset
          </button>
        )}
      </span>
    );
  }

  const title = `${health.consecutive_failures} failure${health.consecutive_failures > 1 ? 's' : ''}${health.last_error ? ` — ${health.last_error}` : ''}${health.last_success_at ? ` · last ok ${relativeTime(health.last_success_at)}` : ''}`;

  return (
    <span className="inline-flex items-center gap-1 ms-1.5" title={title}>
      <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />
      <span className="text-[10px] text-amber-400/70">
        {health.consecutive_failures}x
      </span>
    </span>
  );
}
