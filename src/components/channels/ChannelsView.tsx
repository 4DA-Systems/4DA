import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import { ChannelCard } from './ChannelCard';
import { ChannelContent } from './ChannelContent';
import { CreateChannelModal } from './CreateChannelModal';

export function ChannelsView() {
  const { t } = useTranslation();
  const { channels, channelsLoading, activeChannelId } = useAppStore(
    useShallow(s => ({
      channels: s.channels,
      channelsLoading: s.channelsLoading,
      activeChannelId: s.activeChannelId,
    }))
  );
  const loadChannels = useAppStore(s => s.loadChannels);
  const selectChannel = useAppStore(s => s.selectChannel);
  const [showCreate, setShowCreate] = useState(false);

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
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-sm font-semibold text-white tracking-wide">{t('nav.channels')}</h2>
          <button
            onClick={() => setShowCreate(true)}
            className="w-7 h-7 flex items-center justify-center rounded-lg bg-bg-tertiary border border-border text-text-secondary hover:text-white hover:border-white/30 transition-colors"
            aria-label={t('channels.createTitle')}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
        </div>
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

      <CreateChannelModal open={showCreate} onClose={() => setShowCreate(false)} />
    </div>
  );
}
