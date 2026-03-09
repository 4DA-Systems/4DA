import { useState, useCallback, useEffect, useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';

interface SourcePreview {
  count: number;
  topTitles: string[];
}

interface CreateChannelModalProps {
  open: boolean;
  onClose: () => void;
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9\s-]/g, '')
    .replace(/[\s]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '');
}

export function CreateChannelModal({ open, onClose }: CreateChannelModalProps) {
  const { t } = useTranslation();
  const loadChannels = useAppStore((s) => s.loadChannels);

  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [topics, setTopics] = useState<string[]>([]);
  const [topicInput, setTopicInput] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sourcePreview, setSourcePreview] = useState<SourcePreview | null>(null);
  const previewTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const slug = slugify(title);
  const canSubmit = title.trim().length > 0 && topics.length > 0 && !submitting;

  // Debounced source preview when topics change
  useEffect(() => {
    if (topics.length === 0) {
      setSourcePreview(null);
      return;
    }
    if (previewTimer.current) clearTimeout(previewTimer.current);
    previewTimer.current = setTimeout(() => {
      cmd('preview_channel_sources', { topics })
        .then(setSourcePreview)
        .catch(() => setSourcePreview(null));
    }, 400);
    return () => { if (previewTimer.current) clearTimeout(previewTimer.current); };
  }, [topics]);

  const addTopic = useCallback(() => {
    const trimmed = topicInput.trim();
    if (trimmed && !topics.includes(trimmed)) {
      setTopics((prev) => [...prev, trimmed]);
    }
    setTopicInput('');
  }, [topicInput, topics]);

  const removeTopic = useCallback((topic: string) => {
    setTopics((prev) => prev.filter((t) => t !== topic));
  }, []);

  const handleTopicKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        addTopic();
      }
    },
    [addTopic],
  );

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      await cmd('create_custom_channel', {
        slug,
        title: title.trim(),
        description: description.trim(),
        topicQuery: topics,
      });
      loadChannels();
      cmd('auto_render_all_channels').catch((e) => console.warn('CreateChannelModal: auto-render channels failed', e));
      setTitle('');
      setDescription('');
      setTopics([]);
      setTopicInput('');
      onClose();
    } catch (e) {
      setError(`${e}`);
    } finally {
      setSubmitting(false);
    }
  }, [canSubmit, slug, title, description, topics, loadChannels, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      onClick={onClose}
    >
      <div
        className="bg-bg-secondary border border-border rounded-xl w-full max-w-md shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-6 py-4 border-b border-border flex items-center justify-between">
          <h2 className="text-lg font-medium text-white">{t('channels.createTitle')}</h2>
          <button
            onClick={onClose}
            aria-label={t('action.close')}
            className="w-8 h-8 rounded-lg bg-bg-tertiary text-text-muted hover:text-white hover:bg-border flex items-center justify-center transition-all"
          >
            &times;
          </button>
        </div>

        {/* Body */}
        <div className="p-6 space-y-4">
          {/* Title */}
          <div>
            <label className="block text-sm font-medium text-text-secondary mb-1">
              {t('channels.titleLabel')}
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t('channels.titlePlaceholder')}
              className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-[#666] focus:outline-none focus:border-white/30"
            />
            {slug && (
              <p className="mt-1 text-xs text-text-muted font-mono">/{slug}</p>
            )}
          </div>

          {/* Description */}
          <div>
            <label className="block text-sm font-medium text-text-secondary mb-1">
              {t('channels.descriptionLabel')}
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={2}
              className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-[#666] focus:outline-none focus:border-white/30 resize-none"
            />
          </div>

          {/* Topics */}
          <div>
            <label className="block text-sm font-medium text-text-secondary mb-1">
              {t('channels.topicsLabel')}
            </label>
            <input
              type="text"
              value={topicInput}
              onChange={(e) => setTopicInput(e.target.value)}
              onKeyDown={handleTopicKeyDown}
              placeholder={t('channels.topicsPlaceholder')}
              className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-[#666] focus:outline-none focus:border-white/30"
            />
            {topics.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mt-2">
                {topics.map((topic) => (
                  <span
                    key={topic}
                    className="inline-flex items-center gap-1 px-2 py-0.5 bg-bg-tertiary border border-border rounded-md text-xs text-text-secondary"
                  >
                    {topic}
                    <button
                      onClick={() => removeTopic(topic)}
                      className="text-text-muted hover:text-white transition-colors"
                      aria-label={`Remove ${topic}`}
                    >
                      &times;
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Source preview */}
          {sourcePreview && (
            <div className="text-xs text-text-secondary space-y-1">
              {sourcePreview.count > 0 ? (
                <>
                  <p className="text-[#22C55E]">
                    {t('channels.previewFound', { count: sourcePreview.count })}
                  </p>
                  {sourcePreview.topTitles.length > 0 && (
                    <ul className="list-disc list-inside text-text-muted space-y-0.5">
                      {sourcePreview.topTitles.map((title) => (
                        <li key={title} className="truncate">{title}</li>
                      ))}
                    </ul>
                  )}
                </>
              ) : (
                <p className="text-amber-400">
                  {t('channels.previewEmpty')}
                </p>
              )}
            </div>
          )}

          {error && <p className="text-xs text-[#EF4444]">{error}</p>}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-border flex justify-end gap-2">
          <button
            onClick={onClose}
            aria-label={t('action.cancel')}
            className="px-4 py-2 text-sm font-medium text-text-secondary bg-bg-tertiary border border-border rounded-lg hover:text-white transition-colors"
          >
            {t('action.cancel')}
          </button>
          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            aria-label={submitting ? t('action.loading') : t('channels.create')}
            className="px-4 py-2 text-sm font-medium text-black bg-white rounded-lg hover:bg-white/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {submitting ? t('action.loading') : t('channels.create')}
          </button>
        </div>
      </div>
    </div>
  );
}
