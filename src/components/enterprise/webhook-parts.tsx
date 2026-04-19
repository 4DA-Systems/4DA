// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import type { Webhook, WebhookDelivery } from '../../store/enterprise-slice';

// ============================================================================
// Constants (shared)
// ============================================================================

export const WEBHOOK_EVENTS: Record<string, string[]> = {
  signal: ['signal.detected', 'signal.resolved', 'signal.escalated'],
  decision: ['decision.proposed', 'decision.resolved'],
  member: ['member.joined', 'member.left', 'member.role_changed'],
  system: ['briefing.generated', 'alert.triggered', 'audit.anomaly', 'settings.changed'],
};

export const CIRCUIT_BREAKER_THRESHOLD = 10;

// ============================================================================
// Helpers (shared)
// ============================================================================

export function relativeTime(iso: string | null): string {
  if (!iso) return '--';
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}
// Note: relativeTime abbreviations are universally understood; full i18n deferred to avoid signature changes across call sites

function statusConfig(webhook: Webhook, t: (key: string, opts?: Record<string, unknown>) => string): { dot: string; label: string; color: string } {
  if (!webhook.active && webhook.failure_count >= CIRCUIT_BREAKER_THRESHOLD) {
    return { dot: 'bg-red-500', label: t('enterprise.webhooks.statusTripped'), color: 'text-red-400' };
  }
  if (!webhook.active) {
    return { dot: 'bg-gray-500', label: t('enterprise.webhooks.statusDisabled'), color: 'text-text-muted' };
  }
  if (webhook.failure_count > 0) {
    return { dot: 'bg-orange-500', label: t('enterprise.webhooks.statusDegraded', { count: webhook.failure_count }), color: 'text-orange-400' };
  }
  return { dot: 'bg-green-500', label: t('enterprise.webhooks.statusActive'), color: 'text-green-400' };
}

function deliveryStatusBadge(status: string): { bg: string; text: string } {
  switch (status) {
    case 'delivered': return { bg: 'bg-green-500/20 border-green-500/30', text: 'text-green-400' };
    case 'pending': return { bg: 'bg-yellow-500/20 border-yellow-500/30', text: 'text-yellow-400' };
    case 'failed': return { bg: 'bg-red-500/20 border-red-500/30', text: 'text-red-400' };
    case 'exhausted': return { bg: 'bg-gray-500/20 border-gray-500/30', text: 'text-text-muted' };
    default: return { bg: 'bg-gray-500/20 border-gray-500/30', text: 'text-text-muted' };
  }
}

// ============================================================================
// Webhook Card
// ============================================================================

export interface WebhookCardProps {
  webhook: Webhook;
  deliveries: WebhookDelivery[];
  expanded: boolean;
  deleteConfirm: boolean;
  testing: boolean;
  testResult: boolean | null;
  onToggleDeliveries: () => void;
  onTest: () => void;
  onDeleteRequest: () => void;
  onDeleteConfirm: () => void;
  onDeleteCancel: () => void;
}

export function WebhookCard({
  webhook, deliveries, expanded, deleteConfirm,
  testing, testResult, onToggleDeliveries,
  onTest, onDeleteRequest, onDeleteConfirm, onDeleteCancel,
}: WebhookCardProps) {
  const { t } = useTranslation();
  const status = statusConfig(webhook, t);
  const isCircuitBroken = webhook.failure_count >= CIRCUIT_BREAKER_THRESHOLD;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Circuit Breaker Banner */}
      {isCircuitBroken && (
        <div className="px-4 py-2.5 bg-red-500/10 border-b border-red-500/30 flex items-center justify-between gap-3">
          <p className="text-xs text-red-400">
            {t('enterprise.webhooks.circuitBroken')}
          </p>
          <button
            onClick={onTest}
            className="px-3 py-1 text-[11px] font-medium text-red-400 border border-red-500/30 rounded hover:bg-red-500/20 transition-colors flex-shrink-0"
          >
            {t('enterprise.webhooks.reenable')}
          </button>
        </div>
      )}

      {/* Card Body */}
      <div className="px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          {/* Left: Info */}
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 mb-1">
              <span className={`w-2 h-2 rounded-full flex-shrink-0 ${status.dot}`} />
              <h3 className="text-sm font-semibold text-white truncate">{webhook.name}</h3>
              <span className={`text-[10px] ${status.color}`}>{status.label}</span>
            </div>

            <p className="text-xs font-mono text-text-muted truncate mb-2" title={webhook.url}>
              {webhook.url}
            </p>

            {/* Event Tags */}
            <div className="flex flex-wrap gap-1 mb-2">
              {webhook.events.map(event => (
                <span
                  key={event}
                  className="px-1.5 py-0.5 text-[10px] bg-bg-tertiary text-text-secondary border border-border rounded"
                >
                  {event}
                </span>
              ))}
            </div>

            {/* Meta Row */}
            <div className="flex items-center gap-3 text-[10px] text-text-muted">
              {webhook.last_fired_at && (
                <span>
                  {t('enterprise.webhooks.lastFired')}: {relativeTime(webhook.last_fired_at)}
                </span>
              )}
              {webhook.last_status_code != null && (
                <span className={webhook.last_status_code >= 200 && webhook.last_status_code < 300 ? 'text-green-400' : 'text-red-400'}>
                  HTTP {webhook.last_status_code}
                </span>
              )}
              <span>
                {t('enterprise.webhooks.created')}: {relativeTime(webhook.created_at)}
              </span>
            </div>
          </div>

          {/* Right: Actions */}
          <div className="flex items-center gap-1.5 flex-shrink-0">
            {/* Test */}
            <button
              onClick={onTest}
              disabled={testing}
              aria-label={t('enterprise.webhooks.test')}
              className={`px-2.5 py-1.5 text-[11px] rounded-lg border transition-all ${
                testResult === true
                  ? 'bg-green-500/20 border-green-500/30 text-green-400'
                  : testResult === false
                    ? 'bg-red-500/20 border-red-500/30 text-red-400'
                    : 'bg-bg-tertiary border-border text-text-secondary hover:text-white hover:border-[#3A3A3A]'
              } disabled:opacity-50`}
            >
              {testing
                ? t('enterprise.webhooks.testing')
                : testResult === true
                  ? t('enterprise.webhooks.testPassed')
                  : testResult === false
                    ? t('enterprise.webhooks.testFailed')
                    : t('enterprise.webhooks.test')}
            </button>

            {/* Deliveries */}
            <button
              onClick={onToggleDeliveries}
              aria-expanded={expanded}
              aria-label={t('enterprise.webhooks.deliveries')}
              className={`px-2.5 py-1.5 text-[11px] rounded-lg border transition-all ${
                expanded
                  ? 'bg-white/10 border-white/20 text-white'
                  : 'bg-bg-tertiary border-border text-text-secondary hover:text-white hover:border-[#3A3A3A]'
              }`}
            >
              {t('enterprise.webhooks.deliveries')}
            </button>

            {/* Delete */}
            {deleteConfirm ? (
              <div className="flex items-center gap-1">
                <button
                  onClick={onDeleteConfirm}
                  className="px-2.5 py-1.5 text-[11px] bg-red-500/20 border border-red-500/30 text-red-400 rounded-lg hover:bg-red-500/30 transition-all"
                >
                  {t('enterprise.webhooks.confirmDelete')}
                </button>
                <button
                  onClick={onDeleteCancel}
                  className="px-2.5 py-1.5 text-[11px] bg-bg-tertiary border border-border text-text-muted rounded-lg hover:text-white transition-all"
                >
                  {t('enterprise.webhooks.cancelDelete')}
                </button>
              </div>
            ) : (
              <button
                onClick={onDeleteRequest}
                aria-label={t('enterprise.webhooks.delete')}
                className="px-2.5 py-1.5 text-[11px] bg-bg-tertiary border border-border text-text-muted rounded-lg hover:text-red-400 hover:border-red-500/30 transition-all"
              >
                {t('enterprise.webhooks.delete')}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Delivery History */}
      {expanded && (
        <DeliveryTable deliveries={deliveries} />
      )}
    </div>
  );
}

// ============================================================================
// Delivery Table
// ============================================================================

function DeliveryTable({ deliveries }: { deliveries: WebhookDelivery[] }) {
  const { t } = useTranslation();

  if (deliveries.length === 0) {
    return (
      <div className="px-4 py-4 border-t border-border text-center">
        <p className="text-xs text-text-muted">
          {t('enterprise.webhooks.noDeliveries')}
        </p>
      </div>
    );
  }

  return (
    <div className="border-t border-border overflow-x-auto">
      <table className="w-full text-xs" role="table">
        <thead>
          <tr className="border-b border-border bg-bg-tertiary/50">
            <th scope="col" className="text-start px-4 py-2 text-text-muted font-medium">
              {t('enterprise.webhooks.colTime')}
            </th>
            <th scope="col" className="text-start px-4 py-2 text-text-muted font-medium">
              {t('enterprise.webhooks.colEvent')}
            </th>
            <th scope="col" className="text-start px-4 py-2 text-text-muted font-medium">
              {t('enterprise.webhooks.colStatus')}
            </th>
            <th scope="col" className="text-start px-4 py-2 text-text-muted font-medium">
              {t('enterprise.webhooks.colHttpCode')}
            </th>
            <th scope="col" className="text-start px-4 py-2 text-text-muted font-medium">
              {t('enterprise.webhooks.colAttempts')}
            </th>
          </tr>
        </thead>
        <tbody>
          {deliveries.map(delivery => {
            const badge = deliveryStatusBadge(delivery.status);
            return (
              <tr key={delivery.id} className="border-b border-border/50 hover:bg-bg-tertiary/30 transition-colors">
                <td className="px-4 py-2 text-text-secondary whitespace-nowrap">
                  {relativeTime(delivery.created_at)}
                </td>
                <td className="px-4 py-2 text-text-secondary font-mono">
                  {delivery.event_type}
                </td>
                <td className="px-4 py-2">
                  <span className={`inline-block px-1.5 py-0.5 text-[10px] rounded border ${badge.bg} ${badge.text}`}>
                    {delivery.status}
                  </span>
                </td>
                <td className="px-4 py-2 text-text-muted font-mono">
                  {delivery.http_status ?? '--'}
                </td>
                <td className="px-4 py-2 text-text-muted">
                  {delivery.attempt_count}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
