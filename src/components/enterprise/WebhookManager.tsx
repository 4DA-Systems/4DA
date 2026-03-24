import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { WebhookCard, WEBHOOK_EVENTS } from './webhook-parts';

// ============================================================================
// Main Component
// ============================================================================

export function WebhookManager() {
  const { t } = useTranslation();

  const webhooks = useAppStore(s => s.webhooks);
  const webhooksLoading = useAppStore(s => s.webhooksLoading);
  const webhookDeliveries = useAppStore(s => s.webhookDeliveries);
  const loadWebhooks = useAppStore(s => s.loadWebhooks);
  const registerWebhook = useAppStore(s => s.registerWebhook);
  const deleteWebhook = useAppStore(s => s.deleteWebhook);
  const testWebhook = useAppStore(s => s.testWebhook);
  const loadWebhookDeliveries = useAppStore(s => s.loadWebhookDeliveries);

  const [showForm, setShowForm] = useState(false);
  const [expandedDeliveries, setExpandedDeliveries] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{ id: string; ok: boolean } | null>(null);

  useEffect(() => {
    loadWebhooks();
  }, [loadWebhooks]);

  const handleToggleDeliveries = useCallback(async (webhookId: string) => {
    if (expandedDeliveries === webhookId) {
      setExpandedDeliveries(null);
      return;
    }
    setExpandedDeliveries(webhookId);
    await loadWebhookDeliveries(webhookId, 20);
  }, [expandedDeliveries, loadWebhookDeliveries]);

  const handleTest = useCallback(async (webhookId: string) => {
    setTestingId(webhookId);
    setTestResult(null);
    const ok = await testWebhook(webhookId);
    setTestResult({ id: webhookId, ok });
    setTestingId(null);
    setTimeout(() => setTestResult(null), 4000);
  }, [testWebhook]);

  const handleDelete = useCallback(async (webhookId: string) => {
    await deleteWebhook(webhookId);
    setDeleteConfirm(null);
  }, [deleteWebhook]);

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold text-white">
            {t('enterprise.webhooks.title', 'Webhooks')}
          </h2>
          <span className="px-2 py-0.5 text-[10px] font-medium bg-green-500/15 text-green-400 border border-green-500/30 rounded">
            {t('enterprise.webhooks.enterprise', 'Enterprise')}
          </span>
        </div>
        <button
          onClick={() => setShowForm(prev => !prev)}
          className="px-3 py-1.5 text-sm font-medium text-black bg-white rounded-lg hover:bg-gray-200 transition-colors"
        >
          {showForm
            ? t('enterprise.webhooks.cancel', 'Cancel')
            : t('enterprise.webhooks.addWebhook', 'Add Webhook')}
        </button>
      </div>

      {/* Add Webhook Form */}
      {showForm && (
        <WebhookForm
          onSubmit={async (name, url, events) => {
            const result = await registerWebhook(name, url, events);
            if (result.ok) setShowForm(false);
            return result;
          }}
          onCancel={() => setShowForm(false)}
        />
      )}

      {/* Loading State */}
      {webhooksLoading && webhooks.length === 0 && (
        <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
          <p className="text-text-muted text-sm">{t('enterprise.webhooks.loading', 'Loading webhooks...')}</p>
        </div>
      )}

      {/* Empty State */}
      {!webhooksLoading && webhooks.length === 0 && (
        <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
          <div className="text-2xl mb-2 text-text-muted">{"{ }"}</div>
          <p className="text-text-secondary text-sm mb-1">
            {t('enterprise.webhooks.emptyTitle', 'No webhooks configured')}
          </p>
          <p className="text-text-muted text-xs">
            {t('enterprise.webhooks.emptyDesc', 'Add a webhook to receive real-time notifications for team events.')}
          </p>
        </div>
      )}

      {/* Webhook List */}
      <div className="space-y-3">
        {webhooks.map(webhook => (
          <WebhookCard
            key={webhook.id}
            webhook={webhook}
            deliveries={webhookDeliveries[webhook.id] ?? []}
            expanded={expandedDeliveries === webhook.id}
            deleteConfirm={deleteConfirm === webhook.id}
            testing={testingId === webhook.id}
            testResult={testResult?.id === webhook.id ? testResult.ok : null}
            onToggleDeliveries={() => handleToggleDeliveries(webhook.id)}
            onTest={() => handleTest(webhook.id)}
            onDeleteRequest={() => setDeleteConfirm(webhook.id)}
            onDeleteConfirm={() => handleDelete(webhook.id)}
            onDeleteCancel={() => setDeleteConfirm(null)}
          />
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// Webhook Form (inline, kept in main file since it couples to WebhookManager)
// ============================================================================

interface WebhookFormProps {
  onSubmit: (name: string, url: string, events: string[]) => Promise<{ ok: boolean; error?: string }>;
  onCancel: () => void;
}

function WebhookForm({ onSubmit, onCancel }: WebhookFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [selectedEvents, setSelectedEvents] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggleEvent = useCallback((event: string) => {
    setSelectedEvents(prev =>
      prev.includes(event) ? prev.filter(e => e !== event) : [...prev, event]
    );
  }, []);

  const toggleCategory = useCallback((events: string[]) => {
    setSelectedEvents(prev => {
      const allSelected = events.every(e => prev.includes(e));
      if (allSelected) return prev.filter(e => !events.includes(e));
      return [...new Set([...prev, ...events])];
    });
  }, []);

  const isValidUrl = useMemo(() => {
    if (!url) return true;
    try {
      const parsed = new URL(url);
      return parsed.protocol === 'https:' || parsed.protocol === 'http:';
    } catch {
      return false;
    }
  }, [url]);

  const canSubmit = name.trim() && url.trim() && isValidUrl && selectedEvents.length > 0 && !submitting;

  const handleSubmit = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    const result = await onSubmit(name.trim(), url.trim(), selectedEvents);
    setSubmitting(false);
    if (!result.ok) {
      setError(result.error ?? t('enterprise.webhooks.saveFailed', 'Failed to save webhook'));
    }
  };

  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-4 space-y-4">
      <h3 className="text-sm font-medium text-white">
        {t('enterprise.webhooks.newWebhook', 'New Webhook')}
      </h3>

      {/* Name */}
      <div>
        <label htmlFor="webhook-name" className="block text-xs text-text-secondary mb-1.5">
          {t('enterprise.webhooks.name', 'Name')}
        </label>
        <input
          id="webhook-name"
          type="text"
          value={name}
          onChange={e => setName(e.target.value)}
          placeholder={t('enterprise.webhooks.namePlaceholder', 'e.g. Slack Notifications')}
          className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-white/30 focus:outline-none transition-colors"
        />
      </div>

      {/* URL */}
      <div>
        <label htmlFor="webhook-url" className="block text-xs text-text-secondary mb-1.5">
          {t('enterprise.webhooks.url', 'Endpoint URL')}
        </label>
        <input
          id="webhook-url"
          type="url"
          value={url}
          onChange={e => setUrl(e.target.value)}
          placeholder="https://example.com/webhook"
          className={`w-full px-3 py-2 bg-bg-primary border rounded-lg text-sm text-white placeholder:text-text-muted font-mono focus:outline-none transition-colors ${
            !isValidUrl ? 'border-red-500/50 focus:border-red-500/80' : 'border-border focus:border-white/30'
          }`}
        />
        {!isValidUrl && (
          <p className="text-[10px] text-red-400 mt-1">
            {t('enterprise.webhooks.invalidUrl', 'URL must start with https:// or http://')}
          </p>
        )}
        {isValidUrl && url && !url.startsWith('https://') && (
          <p className="text-[10px] text-yellow-400 mt-1">
            {t('enterprise.webhooks.httpsRecommended', 'HTTPS is recommended for production webhooks')}
          </p>
        )}
      </div>

      {/* Events */}
      <div>
        <span className="block text-xs text-text-secondary mb-2">
          {t('enterprise.webhooks.events', 'Events')}
        </span>
        <div className="space-y-3">
          {Object.entries(WEBHOOK_EVENTS).map(([category, events]) => (
            <fieldset key={category} className="space-y-1.5">
              <legend className="text-[11px] font-medium text-text-muted uppercase tracking-wide">
                <button
                  type="button"
                  onClick={() => toggleCategory(events)}
                  className="text-text-muted hover:text-white transition-colors underline decoration-dotted"
                  aria-label={t('enterprise.webhooks.toggleCategory', `Toggle all ${category} events`)}
                >
                  {category}
                </button>
              </legend>
              <div className="flex flex-wrap gap-2 ps-1">
                {events.map(event => {
                  const checked = selectedEvents.includes(event);
                  return (
                    <label
                      key={event}
                      className={`flex items-center gap-1.5 px-2 py-1 rounded-lg border text-xs cursor-pointer transition-all ${
                        checked
                          ? 'bg-white/10 border-white/20 text-white'
                          : 'bg-bg-tertiary border-border text-text-muted hover:text-text-secondary hover:border-[#3A3A3A]'
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => toggleEvent(event)}
                        className="sr-only"
                        aria-label={event}
                      />
                      <span className={`w-3 h-3 rounded border flex items-center justify-center flex-shrink-0 ${
                        checked ? 'bg-white border-white' : 'border-[#3A3A3A]'
                      }`}>
                        {checked && (
                          <svg className="w-2 h-2 text-black" viewBox="0 0 12 12" fill="none">
                            <path d="M2 6l3 3 5-5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                          </svg>
                        )}
                      </span>
                      {event}
                    </label>
                  );
                })}
              </div>
            </fieldset>
          ))}
        </div>
        {selectedEvents.length > 0 && (
          <p className="text-[10px] text-text-muted mt-2">
            {t('enterprise.webhooks.selectedCount', '{{count}} events selected', { count: selectedEvents.length })}
          </p>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="p-2.5 rounded-lg bg-red-500/10 border border-red-500/30">
          <p className="text-xs text-red-400">{error}</p>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2 pt-1">
        <button
          onClick={handleSubmit}
          disabled={!canSubmit}
          className="px-4 py-2 text-sm font-medium text-black bg-white rounded-lg hover:bg-gray-200 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {submitting
            ? t('enterprise.webhooks.saving', 'Saving...')
            : t('enterprise.webhooks.save', 'Save Webhook')}
        </button>
        <button
          onClick={onCancel}
          className="px-4 py-2 text-sm text-text-secondary hover:text-white transition-colors"
        >
          {t('enterprise.webhooks.cancel', 'Cancel')}
        </button>
      </div>
    </div>
  );
}
