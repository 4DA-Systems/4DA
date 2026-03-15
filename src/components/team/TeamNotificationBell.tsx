import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const SEVERITY_COLORS: Record<string, string> = {
  critical: 'bg-[#EF4444]/15 text-[#EF4444]',
  warning: 'bg-[#F97316]/15 text-[#F97316]',
  info: 'bg-[#3B82F6]/10 text-[#3B82F6]',
};

const TYPE_ICONS: Record<string, string> = {
  signal_detected: '\u26A0',     // warning sign
  decision_proposed: '\u2696',   // scales
  decision_resolved: '\u2714',   // check
  member_joined: '\u2795',       // plus
  member_left: '\u2796',         // minus
  source_shared: '\u2B50',       // star
};

export function TeamNotificationBell() {
  const { t } = useTranslation();
  const notifications = useAppStore(s => s.notifications);
  const notificationSummary = useAppStore(s => s.notificationSummary);
  const loadNotifications = useAppStore(s => s.loadNotifications);
  const loadNotificationSummary = useAppStore(s => s.loadNotificationSummary);
  const markNotificationRead = useAppStore(s => s.markNotificationRead);
  const markAllNotificationsRead = useAppStore(s => s.markAllNotificationsRead);
  const dismissNotification = useAppStore(s => s.dismissNotification);
  const tier = useAppStore(s => s.tier);
  const teamStatus = useAppStore(s => s.teamStatus);

  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  const isTeam = (tier === 'team' || tier === 'enterprise') && teamStatus?.enabled;

  useEffect(() => {
    if (isTeam) {
      loadNotificationSummary();
      // Poll every 60s
      const interval = setInterval(loadNotificationSummary, 60000);
      return () => clearInterval(interval);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isTeam]);

  useEffect(() => {
    if (open && isTeam) {
      loadNotifications();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as globalThis.Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  if (!isTeam) return null;

  const unreadCount = notificationSummary?.total_unread ?? 0;

  const handleClick = (notif: typeof notifications[0]) => {
    if (!notif.read) {
      markNotificationRead(notif.id);
    }
  };

  return (
    <div className="relative" ref={panelRef}>
      {/* Bell Button */}
      <button
        onClick={() => setOpen(!open)}
        className="relative p-1.5 rounded-lg hover:bg-bg-tertiary transition-colors"
        aria-label={t('team.notifications.bell', 'Team notifications')}
        aria-expanded={open}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-text-muted">
          <path
            d="M8 1.5A4.5 4.5 0 003.5 6v2.5L2 10v1h12v-1l-1.5-1.5V6A4.5 4.5 0 008 1.5zM6.5 12a1.5 1.5 0 003 0"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        {unreadCount > 0 && (
          <span className="absolute -top-0.5 -right-0.5 w-4 h-4 bg-[#EF4444] text-white text-[8px] font-bold rounded-full flex items-center justify-center">
            {unreadCount > 9 ? '9+' : unreadCount}
          </span>
        )}
      </button>

      {/* Dropdown Panel */}
      {open && (
        <div className="absolute right-0 top-full mt-1 w-80 bg-bg-secondary border border-border rounded-xl shadow-2xl z-50 overflow-hidden">
          {/* Header */}
          <div className="px-3 py-2.5 border-b border-border flex items-center justify-between">
            <span className="text-xs font-medium text-white">
              {t('team.notifications.title', 'Notifications')}
            </span>
            {unreadCount > 0 && (
              <button
                onClick={markAllNotificationsRead}
                className="text-[10px] text-text-muted hover:text-[#22C55E] transition-colors"
              >
                {t('team.notifications.markAllRead', 'Mark all read')}
              </button>
            )}
          </div>

          {/* Notification List */}
          <div className="max-h-80 overflow-y-auto">
            {notifications.length === 0 ? (
              <p className="text-xs text-text-muted text-center py-8">
                {t('team.notifications.empty', 'No notifications')}
              </p>
            ) : (
              notifications.map(notif => (
                <div
                  key={notif.id}
                  onClick={() => handleClick(notif)}
                  className={`px-3 py-2.5 border-b border-border/30 hover:bg-bg-tertiary/50 transition-colors cursor-pointer ${
                    !notif.read ? 'bg-bg-tertiary/20' : ''
                  }`}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleClick(notif); } }}
                >
                  <div className="flex items-start gap-2">
                    {/* Unread dot */}
                    <div className="mt-1 shrink-0">
                      {!notif.read ? (
                        <div className="w-1.5 h-1.5 rounded-full bg-[#22C55E]" />
                      ) : (
                        <div className="w-1.5 h-1.5" />
                      )}
                    </div>

                    {/* Icon */}
                    <span className="text-xs shrink-0 mt-0.5">
                      {TYPE_ICONS[notif.notification_type] || '\u2022'}
                    </span>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-1.5">
                        <span className="text-xs text-white font-medium truncate">{notif.title}</span>
                        <span className={`text-[9px] px-1 py-0.5 rounded shrink-0 ${SEVERITY_COLORS[notif.severity] || SEVERITY_COLORS.info}`}>
                          {notif.severity}
                        </span>
                      </div>
                      {notif.body && (
                        <p className="text-[10px] text-text-muted mt-0.5 line-clamp-2">{notif.body}</p>
                      )}
                      <span className="text-[9px] text-text-muted mt-0.5 block">
                        {formatRelativeTime(notif.created_at)}
                      </span>
                    </div>

                    {/* Dismiss */}
                    <button
                      onClick={e => { e.stopPropagation(); dismissNotification(notif.id); }}
                      className="text-text-muted hover:text-[#EF4444] transition-colors shrink-0 p-0.5"
                      aria-label="Dismiss notification"
                    >
                      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                        <path d="M2 2l6 6M8 2l-6 6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
                      </svg>
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}
