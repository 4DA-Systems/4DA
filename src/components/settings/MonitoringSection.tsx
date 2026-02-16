import type { MonitoringStatus } from '../../types';

interface MonitoringSectionProps {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (val: number) => void;
  notificationThreshold: string;
  onSetNotificationThreshold: (threshold: string) => void;
  onToggle: () => void;
  onUpdateInterval: () => void;
  onTestNotification: () => void;
}

export function MonitoringSection({
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  notificationThreshold,
  onSetNotificationThreshold,
  onToggle,
  onUpdateInterval,
  onTestNotification,
}: MonitoringSectionProps) {
  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
          <span>&#x1f504;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-white">Background Monitoring</h3>
          <p className="text-xs text-gray-500">Auto-analyze at intervals</p>
        </div>
      </div>

      {monitoring ? (
        <div className="space-y-4">
          <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="flex items-center gap-2">
              {monitoring.enabled ? (
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              ) : (
                <div className="w-2 h-2 bg-gray-600 rounded-full" />
              )}
              <span className="text-sm text-white">
                {monitoring.enabled ? (
                  <span className="text-green-400">Active</span>
                ) : (
                  <span className="text-gray-500">Inactive</span>
                )}
              </span>
              {monitoring.is_checking && (
                <span className="text-xs text-orange-400 ml-2">(checking...)</span>
              )}
            </div>
            <button
              onClick={onToggle}
              className={`px-4 py-2 text-sm rounded-lg transition-all ${
                monitoring.enabled
                  ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
                  : 'bg-green-500/20 text-green-400 hover:bg-green-500/30'
              }`}
            >
              {monitoring.enabled ? 'Stop' : 'Start'}
            </button>
          </div>

          <div className="flex items-center gap-3">
            <label className="text-sm text-gray-400">Every</label>
            <input
              type="number"
              min="5"
              max="1440"
              value={monitoringInterval}
              onChange={(e) => setMonitoringInterval(parseInt(e.target.value) || 30)}
              className="w-20 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white text-center focus:border-orange-500 focus:outline-none"
            />
            <span className="text-sm text-gray-400">minutes</span>
            <button
              onClick={onUpdateInterval}
              className="px-4 py-2 text-sm bg-bg-secondary border border-border text-gray-400 rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
            >
              Update
            </button>
          </div>

          <div className="flex items-center gap-3">
            <label className="text-sm text-gray-400">Notifications</label>
            <select
              value={notificationThreshold}
              onChange={(e) => onSetNotificationThreshold(e.target.value)}
              className="px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none appearance-none cursor-pointer"
              style={{ backgroundImage: 'url("data:image/svg+xml,%3Csvg xmlns=\'http://www.w3.org/2000/svg\' width=\'12\' height=\'12\' fill=\'%23666\' viewBox=\'0 0 16 16\'%3E%3Cpath d=\'M8 11L3 6h10z\'/%3E%3C/svg%3E")', backgroundRepeat: 'no-repeat', backgroundPosition: 'right 0.75rem center', paddingRight: '2rem' }}
            >
              <option value="critical_only">Critical only</option>
              <option value="high_and_above">High and above</option>
              <option value="all">All items</option>
            </select>
          </div>

          <div className="flex items-center justify-between text-xs text-gray-500 px-1">
            <span>Total checks: {monitoring.total_checks}</span>
            {monitoring.last_check_ago && (
              <span>Last: {monitoring.last_check_ago}</span>
            )}
          </div>

          <button
            onClick={onTestNotification}
            className="w-full px-4 py-2.5 text-sm bg-bg-secondary border border-border text-gray-400 rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
          >
            Test Notification
          </button>
        </div>
      ) : (
        <div className="text-xs text-text-muted">Loading monitoring status...</div>
      )}
    </div>
  );
}
