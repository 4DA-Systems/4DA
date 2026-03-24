import { useState } from 'react';
import { useTranslation } from 'react-i18next';

const WEBHOOK_EVENTS = [
  {
    event: 'signal.detected',
    description: 'Fired when a new signal is detected across multiple team members',
    payload: '{ signal_id, signal_type, title, severity, tech_topics, detected_by_count }',
  },
  {
    event: 'signal.resolved',
    description: 'Fired when a team signal is resolved',
    payload: '{ signal_id, resolved_by, resolution_notes }',
  },
  {
    event: 'decision.proposed',
    description: 'Fired when a team member proposes a decision',
    payload: '{ decision_id, title, decision_type, proposed_by }',
  },
  {
    event: 'decision.voted',
    description: 'Fired when a team member votes on a decision',
    payload: '{ decision_id, voter_id, stance }',
  },
  {
    event: 'decision.resolved',
    description: 'Fired when a decision is accepted or rejected',
    payload: '{ decision_id, title, new_status }',
  },
  {
    event: 'member.joined',
    description: 'Fired when a new member joins the team',
    payload: '{ client_id, display_name, role }',
  },
  {
    event: 'member.left',
    description: 'Fired when a member leaves the team',
    payload: '{ client_id, reason }',
  },
  {
    event: 'source.shared',
    description: 'Fired when a source recommendation is shared',
    payload: '{ source_id, source_type, recommendation, shared_by }',
  },
  {
    event: 'admin.policy_changed',
    description: 'Fired when a retention policy is modified',
    payload: '{ resource_type, retention_days }',
  },
  {
    event: 'webhook.tested',
    description: 'Fired when a webhook test is triggered',
    payload: '{ webhook_id, test: true }',
  },
];

export function WebhookDocsPanel() {
  const { t } = useTranslation();
  const [expandedEvent, setExpandedEvent] = useState<string | null>(null);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div>
        <h3 className="text-sm font-medium text-white">
          {t('enterprise.docs.title', 'Webhook & API Reference')}
        </h3>
        <p className="text-[10px] text-text-muted mt-0.5">
          {t('enterprise.docs.description', 'Technical reference for integrating 4DA webhooks with your infrastructure.')}
        </p>
      </div>

      {/* Delivery Format */}
      <div className="bg-bg-primary rounded-lg p-3 border border-border/50">
        <h4 className="text-xs font-medium text-text-secondary mb-2">Delivery Format</h4>
        <div className="text-[10px] text-text-muted space-y-1 font-mono">
          <p>POST &lt;webhook_url&gt;</p>
          <p>Content-Type: application/json</p>
          <p>X-4DA-Signature-256: sha256=&lt;hmac_hex&gt;</p>
          <p>X-4DA-Event: &lt;event_type&gt;</p>
          <p>X-4DA-Delivery: &lt;delivery_uuid&gt;</p>
          <p>User-Agent: 4DA-Webhook/1.0</p>
        </div>
      </div>

      {/* Signature Verification */}
      <div className="bg-bg-primary rounded-lg p-3 border border-border/50">
        <h4 className="text-xs font-medium text-text-secondary mb-2">Signature Verification</h4>
        <p className="text-[10px] text-text-muted mb-2">
          Each delivery is signed with HMAC-SHA256 using the webhook secret. Verify by computing:
        </p>
        <code className="text-[10px] text-[#818CF8] font-mono block bg-bg-tertiary px-2 py-1.5 rounded">
          HMAC-SHA256(secret, request_body) === signature_hex
        </code>
      </div>

      {/* Retry Policy */}
      <div className="bg-bg-primary rounded-lg p-3 border border-border/50">
        <h4 className="text-xs font-medium text-text-secondary mb-2">Retry Policy</h4>
        <div className="text-[10px] text-text-muted space-y-1">
          <p>&#8226; Up to 5 retry attempts with exponential backoff</p>
          <p>&#8226; Schedule: 1 min, 5 min, 30 min, 2 hours, 12 hours</p>
          <p>&#8226; Circuit breaker opens after 5 consecutive failures</p>
          <p>&#8226; Circuit breaker auto-resets after 30 minutes</p>
          <p>&#8226; 2xx responses are treated as successful delivery</p>
        </div>
      </div>

      {/* Event Reference */}
      <div>
        <h4 className="text-xs font-medium text-text-secondary mb-2">
          Event Types ({WEBHOOK_EVENTS.length})
        </h4>
        <div className="space-y-1">
          {WEBHOOK_EVENTS.map(evt => (
            <div key={evt.event} className="bg-bg-primary rounded-lg border border-border/50 overflow-hidden">
              <button
                onClick={() => setExpandedEvent(expandedEvent === evt.event ? null : evt.event)}
                className="w-full px-3 py-2 flex items-center justify-between hover:bg-bg-tertiary/30 transition-colors text-left"
                aria-expanded={expandedEvent === evt.event}
              >
                <code className="text-xs text-accent-gold font-mono">{evt.event}</code>
                <span className={`text-text-muted text-[10px] transition-transform ${expandedEvent === evt.event ? 'rotate-180' : ''}`}>
                  &#9660;
                </span>
              </button>
              {expandedEvent === evt.event && (
                <div className="px-3 py-2 border-t border-border/30">
                  <p className="text-[10px] text-text-secondary mb-1.5">{evt.description}</p>
                  <p className="text-[10px] text-text-muted mb-0.5">Payload:</p>
                  <code className="text-[9px] text-[#818CF8] font-mono block bg-bg-tertiary px-2 py-1 rounded">
                    {evt.payload}
                  </code>
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Payload Envelope */}
      <div className="bg-bg-primary rounded-lg p-3 border border-border/50">
        <h4 className="text-xs font-medium text-text-secondary mb-2">Payload Envelope</h4>
        <pre className="text-[10px] text-[#818CF8] font-mono bg-bg-tertiary p-2 rounded overflow-x-auto">
{`{
  "event": "signal.detected",
  "timestamp": "2026-03-13T12:00:00Z",
  "data": {
    "signal_id": "abc-123",
    "severity": "high",
    ...
  }
}`}
        </pre>
      </div>
    </div>
  );
}
