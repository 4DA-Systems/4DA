// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { ValidationResult } from './useSourceConfig';

interface ValidationFeedbackProps {
  validating: boolean;
  result: ValidationResult;
  onTryFeed: (url: string) => void;
}

/** Inline validation feedback for source addition — shows status, errors, or discovered feeds. */
export function ValidationFeedback({ validating, result, onTryFeed }: ValidationFeedbackProps) {
  if (validating) {
    return <div className="text-xs text-[#8A8A8A] mt-1 animate-pulse">Validating...</div>;
  }

  if (!result) return null;

  if (result.valid) {
    const label = result.feed_title || result.channel_name || 'Source';
    const count = result.item_count ?? result.video_count ?? 0;
    const unit = result.video_count != null ? 'videos' : 'items';
    return (
      <div className="text-xs mt-1 text-[#22C55E]">
        Found: {label} — {count} {unit}
      </div>
    );
  }

  return (
    <div className="text-xs mt-1 text-[#EF4444]">
      <span>{result.message || 'Validation failed'}</span>
      {result.discovered_feeds && result.discovered_feeds.length > 0 && (
        <div className="mt-1 space-y-0.5">
          <span className="text-[#8A8A8A]">Discovered feeds:</span>
          {result.discovered_feeds.map((feed) => (
            <button
              key={feed}
              onClick={() => onTryFeed(feed)}
              className="block text-left text-[#A0A0A0] hover:text-white underline truncate max-w-full"
            >
              {feed}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
