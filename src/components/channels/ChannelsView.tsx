import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { ChannelCard } from './ChannelCard';
import { ChannelContent } from './ChannelContent';

export function ChannelsView() {
  const { t } = useTranslation();
  const channels = useAppStore((s) => s.channels);
  const channelsLoading = useAppStore((s) => s.channelsLoading);
  const activeChannelId = useAppStore((s) => s.activeChannelId);
  const loadChannels = useAppStore((s) => s.loadChannels);
  const selectChannel = useAppStore((s) => s.selectChannel);

  // Load channels on mount
  useEffect(() => {
    loadChannels();
  }, [loadChannels]);

  // Auto-select first channel if none active
  useEffect(() => {
    if (channels.length > 0 && !activeChannelId) {
      selectChannel(channels[0].id);
    }
  }, [channels, activeChannelId, selectChannel]);

  // Full-page loading state when no channels cached yet
  if (channelsLoading && channels.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <div className="w-5 h-5 border-2 border-cyan-500/30 border-t-cyan-500 rounded-full animate-spin" />
          <span className="text-sm text-text-muted">
            {t('action.loading')}
          </span>
        </div>
      </div>
    );
  }

  // Empty state
  if (channels.length === 0) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-sm text-text-muted">
          {t('channels.noChannels')}
        </p>
      </div>
    );
  }

  return (
    <div
      className="grid grid-cols-1 lg:grid-cols-3 gap-6"
      role="tabpanel"
      id="view-panel-channels"
    >
      {/* Sidebar: Channel List */}
      <div className="lg:col-span-1 space-y-2">
        {channels.map((channel) => (
          <ChannelCard
            key={channel.id}
            channel={channel}
            active={channel.id === activeChannelId}
            onClick={() => selectChannel(channel.id)}
          />
        ))}
      </div>

      {/* Content Area */}
      <div className="lg:col-span-2 bg-bg-secondary border border-border rounded-lg p-6">
        <ChannelContent />
      </div>
    </div>
  );
}
