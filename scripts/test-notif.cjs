// Test notification trigger — sends critical/high/low through 4DA
const WebSocket = require('ws');

const priority = process.argv[2] || 'low';
const targetId = process.argv[3];

if (!targetId) {
  console.error('Usage: node test-notif.js <priority> <cdp-target-id>');
  process.exit(1);
}

const PAYLOADS = {
  critical: {
    variant: 'signal', priority: 'critical', signal_type: 'security_alert',
    title: 'CVE-2026-1234 in SQLite: RCE vulnerability',
    action: 'Update dependency immediately', source: 'cve',
    matched_deps: ['sqlite', 'rusqlite'], time_ago: 'just now', item_id: 42,
  },
  high: {
    variant: 'signal', priority: 'high', signal_type: 'breaking_change',
    title: 'React 20 drops class components — migration guide',
    action: 'Check migration path', source: 'hackernews',
    matched_deps: ['react', 'react-dom'], time_ago: '5m ago', item_id: null,
  },
  medium: {
    variant: 'signal', priority: 'medium', signal_type: 'tool_discovery',
    title: 'Show HN: A new Rust testing framework',
    action: 'via hackernews', source: 'hackernews',
    matched_deps: ['rust'], time_ago: '12m ago', item_id: null,
  },
  low: null, // use trigger_notification_test
};

const ws = new WebSocket(`ws://localhost:9222/devtools/page/${targetId}`);
ws.on('error', (e) => { console.error('WS error:', e.message); process.exit(1); });
ws.on('open', () => {
  // Step 1: trigger_notification_test to show the window via Rust
  ws.send(JSON.stringify({
    id: 1,
    method: 'Runtime.evaluate',
    params: {
      expression: 'window.__TAURI__.core.invoke("trigger_notification_test")',
      awaitPromise: true
    }
  }));
});

ws.on('message', (raw) => {
  const msg = JSON.parse(raw);
  if (msg.id === 1) {
    console.log('Window shown');
    const payload = PAYLOADS[priority];
    if (!payload) {
      // Low — already triggered by trigger_notification_test
      console.log('Low notification (default)');
      ws.close();
      process.exit(0);
    }
    // Step 2: Send the priority-specific data to override
    const expr = `window.__TAURI__.event.emit_to("notification", "notification-data", ${JSON.stringify(payload)})`;
    ws.send(JSON.stringify({
      id: 2,
      method: 'Runtime.evaluate',
      params: { expression: expr, awaitPromise: true }
    }));
  }
  if (msg.id === 2) {
    console.log(`${priority} notification sent`);
    ws.close();
    process.exit(0);
  }
});

setTimeout(() => process.exit(0), 5000);
