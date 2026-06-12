// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import type { ValidationResult } from './source-config-types';

interface ValidationFeedbackProps {
  validating: boolean;
  result: ValidationResult;
  onTryFeed: (url: string) => void;
}

/** Inline validation feedback for source addition — shows status, errors, or discovered feeds. */
export function ValidationFeedback({ validating, result, onTryFeed }: ValidationFeedbackProps) {
  const { t } = useTranslation();

  if (validating) {
    return (
      <div className="text-xs text-text-muted mt-1 animate-pulse">{t('action.validating')}</div>
    );
  }

  if (!result) return null;

  if (result.valid) {
    const label = result.feed_title || result.channel_name || t('validation.source');
    const count = result.item_count ?? result.video_count ?? 0;
    const unit = result.video_count != null ? t('validation.videos') : t('validation.items');
    return (
      <div className="text-xs mt-1 text-success">
        {t('validation.found', { label, count, unit })}
      </div>
    );
  }

  return (
    <div className="text-xs mt-1 text-error">
      <span>{result.message || t('validation.failed')}</span>
      {result.discovered_feeds && result.discovered_feeds.length > 0 && (
        <div className="mt-1 space-y-0.5">
          <span className="text-text-muted">{t('validation.discoveredFeeds')}</span>
          {result.discovered_feeds.map((feed) => (
            <button
              key={feed}
              onClick={() => onTryFeed(feed)}
              className="block text-left text-text-secondary hover:text-text-primary underline truncate max-w-full"
            >
              {feed}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
