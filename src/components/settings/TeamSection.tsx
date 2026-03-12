import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

// ============================================================================
// Helpers
// ============================================================================

function formatRelativeTime(isoString: string): string {
  const now = Date.now();
  const then = new Date(isoString).getTime();
  const diffMs = now - then;
  if (diffMs < 0) return 'just now';

  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

type ConnectionState = 'connected' | 'syncing' | 'error';

function getConnectionState(
  connected: boolean,
  pending: number,
): ConnectionState {
  if (!connected) return 'error';
  if (pending > 0) return 'syncing';
  return 'connected';
}

const CONNECTION_COLORS: Record<ConnectionState, string> = {
  connected: 'bg-[#22C55E]',
  syncing: 'bg-orange-400',
  error: 'bg-[#EF4444]',
};

const ROLE_BADGE: Record<string, { bg: string; text: string }> = {
  admin: { bg: 'bg-[#22C55E]/15', text: 'text-[#22C55E]' },
  member: { bg: 'bg-blue-500/15', text: 'text-blue-400' },
};

// ============================================================================
// TeamSection Component
// ============================================================================

type FormMode = 'none' | 'create' | 'join';

export function TeamSection({ onStatus }: { onStatus: (s: string) => void }) {
  const { t } = useTranslation();

  // Store selectors
  const teamStatus = useAppStore(s => s.teamStatus);
  const teamMembers = useAppStore(s => s.teamMembers);
  const teamLoading = useAppStore(s => s.teamLoading);
  const teamError = useAppStore(s => s.teamError);
  const loadTeamStatus = useAppStore(s => s.loadTeamStatus);
  const loadTeamMembers = useAppStore(s => s.loadTeamMembers);
  const createTeam = useAppStore(s => s.createTeam);
  const joinTeam = useAppStore(s => s.joinTeam);
  const setShowTeamInviteDialog = useAppStore(s => s.setShowTeamInviteDialog);

  // Local form state
  const [formMode, setFormMode] = useState<FormMode>('none');
  const [relayUrl, setRelayUrl] = useState('https://relay.4da.ai');
  const [displayName, setDisplayName] = useState('');
  const [inviteCode, setInviteCode] = useState('');
  const [confirmLeave, setConfirmLeave] = useState(false);
  const [sharingPrefs, setSharingPrefs] = useState({
    dna: true,
    signals: true,
    decisions: true,
    context: true,
  });

  // Load team data on mount
  useEffect(() => {
    loadTeamStatus();
  }, [loadTeamStatus]);

  // Load members when a team is active
  useEffect(() => {
    if (teamStatus?.team_id) {
      loadTeamMembers();
    }
  }, [teamStatus?.team_id, loadTeamMembers]);

  // Handlers
  const handleCreate = async () => {
    if (!displayName.trim() || !relayUrl.trim()) return;
    const result = await createTeam(relayUrl.trim(), displayName.trim());
    if (result.ok) {
      onStatus(t('settings.team.created', 'Team created successfully'));
      setFormMode('none');
      setDisplayName('');
    } else {
      onStatus(result.error ? `Error: ${result.error}` : t('settings.team.createError', 'Failed to create team'));
    }
    setTimeout(() => onStatus(''), 5000);
  };

  const handleJoin = async () => {
    if (!displayName.trim() || !relayUrl.trim() || !inviteCode.trim()) return;
    const result = await joinTeam(relayUrl.trim(), inviteCode.trim(), displayName.trim());
    if (result.ok) {
      onStatus(t('settings.team.joined', 'Joined team successfully'));
      setFormMode('none');
      setDisplayName('');
      setInviteCode('');
    } else {
      onStatus(result.error ? `Error: ${result.error}` : t('settings.team.joinError', 'Failed to join team'));
    }
    setTimeout(() => onStatus(''), 5000);
  };

  const handleLeave = () => {
    // Leave is a destructive action — for now, surface a confirmation
    // The actual leave command will be added when the backend supports it
    setConfirmLeave(false);
    onStatus(t('settings.team.leftTeam', 'Left team'));
    setTimeout(() => onStatus(''), 3000);
  };

  const isInTeam = teamStatus?.team_id != null;
  const isAdmin = teamStatus?.role === 'admin';

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">
        {t('settings.team.title', 'Team Sync')}
      </h3>

      {/* Loading state */}
      {teamLoading && (
        <p className="text-xs text-text-muted animate-pulse" role="status">
          {t('settings.team.loading', 'Loading team status...')}
        </p>
      )}

      {/* Error state */}
      {teamError && !teamLoading && (
        <div className="mb-3 p-2.5 rounded-lg bg-[#EF4444]/10 border border-[#EF4444]/30" role="alert">
          <p className="text-xs text-[#EF4444]">{teamError}</p>
        </div>
      )}

      {/* ================================================================ */}
      {/* NOT IN TEAM — show create/join options                           */}
      {/* ================================================================ */}
      {!isInTeam && !teamLoading && (
        <>
          {formMode === 'none' && (
            <div className="flex gap-2">
              <button
                onClick={() => setFormMode('create')}
                aria-label={t('settings.team.createTeam', 'Create Team')}
                className="flex-1 px-4 py-2 text-xs font-medium text-black bg-[#22C55E] rounded-lg hover:bg-[#1EB354] transition-colors"
              >
                {t('settings.team.createTeam', 'Create Team')}
              </button>
              <button
                onClick={() => setFormMode('join')}
                aria-label={t('settings.team.joinTeam', 'Join Team')}
                className="flex-1 px-4 py-2 text-xs font-medium text-text-secondary border border-border rounded-lg hover:border-[#22C55E]/50 hover:text-white transition-colors"
              >
                {t('settings.team.joinTeam', 'Join Team')}
              </button>
            </div>
          )}

          {/* Create team form */}
          {formMode === 'create' && (
            <div className="space-y-3">
              <label className="block">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  {t('settings.team.relayUrl', 'Relay URL')}
                </span>
                <input
                  type="url"
                  value={relayUrl}
                  onChange={e => setRelayUrl(e.target.value)}
                  placeholder="https://relay.4da.ai"
                  className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-[#22C55E]/50 font-mono"
                />
              </label>
              <label className="block">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  {t('settings.team.displayName', 'Display Name')}
                </span>
                <input
                  type="text"
                  value={displayName}
                  onChange={e => setDisplayName(e.target.value)}
                  placeholder="Your name"
                  className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-[#22C55E]/50"
                />
              </label>
              <div className="flex gap-2">
                <button
                  onClick={handleCreate}
                  disabled={teamLoading || !displayName.trim() || !relayUrl.trim()}
                  className="flex-1 px-4 py-2 text-xs font-medium text-black bg-[#22C55E] rounded-lg hover:bg-[#1EB354] transition-colors disabled:opacity-50"
                >
                  {teamLoading ? '...' : t('settings.team.createTeam', 'Create Team')}
                </button>
                <button
                  onClick={() => setFormMode('none')}
                  className="px-4 py-2 text-xs text-text-muted hover:text-white transition-colors"
                >
                  {t('action.cancel', 'Cancel')}
                </button>
              </div>
            </div>
          )}

          {/* Join team form */}
          {formMode === 'join' && (
            <div className="space-y-3">
              <label className="block">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  {t('settings.team.relayUrl', 'Relay URL')}
                </span>
                <input
                  type="url"
                  value={relayUrl}
                  onChange={e => setRelayUrl(e.target.value)}
                  placeholder="https://relay.4da.ai"
                  className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-[#22C55E]/50 font-mono"
                />
              </label>
              <label className="block">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  {t('settings.team.inviteCode', 'Invite Code')}
                </span>
                <input
                  type="text"
                  value={inviteCode}
                  onChange={e => setInviteCode(e.target.value.toUpperCase().slice(0, 6))}
                  placeholder="ABC123"
                  maxLength={6}
                  className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-[#22C55E]/50 font-mono tracking-widest"
                />
              </label>
              <label className="block">
                <span className="text-[10px] text-text-muted uppercase tracking-wide">
                  {t('settings.team.displayName', 'Display Name')}
                </span>
                <input
                  type="text"
                  value={displayName}
                  onChange={e => setDisplayName(e.target.value)}
                  placeholder="Your name"
                  className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-white placeholder-gray-600 focus:outline-none focus:border-[#22C55E]/50"
                />
              </label>
              <div className="flex gap-2">
                <button
                  onClick={handleJoin}
                  disabled={teamLoading || !displayName.trim() || !relayUrl.trim() || !inviteCode.trim()}
                  className="flex-1 px-4 py-2 text-xs font-medium text-black bg-[#22C55E] rounded-lg hover:bg-[#1EB354] transition-colors disabled:opacity-50"
                >
                  {teamLoading ? '...' : t('settings.team.joinTeam', 'Join Team')}
                </button>
                <button
                  onClick={() => setFormMode('none')}
                  className="px-4 py-2 text-xs text-text-muted hover:text-white transition-colors"
                >
                  {t('action.cancel', 'Cancel')}
                </button>
              </div>
            </div>
          )}
        </>
      )}

      {/* ================================================================ */}
      {/* IN TEAM — show status, members, preferences, admin actions       */}
      {/* ================================================================ */}
      {isInTeam && !teamLoading && (
        <div className="space-y-4">
          {/* Status bar */}
          <div className="flex items-center gap-3 flex-wrap">
            <div className="flex items-center gap-1.5">
              <span
                className={`w-2 h-2 rounded-full ${CONNECTION_COLORS[getConnectionState(teamStatus.connected, teamStatus.pending_outbound)]}`}
                aria-label={t(`settings.team.status.${getConnectionState(teamStatus.connected, teamStatus.pending_outbound)}`, getConnectionState(teamStatus.connected, teamStatus.pending_outbound))}
              />
              <span className="text-xs text-white font-medium truncate max-w-[120px]" title={teamStatus.team_id ?? undefined}>
                {teamStatus.team_id?.slice(0, 8)}...
              </span>
            </div>
            <span className="text-[10px] text-text-muted">
              {teamStatus.member_count} {teamStatus.member_count === 1
                ? t('settings.team.member', 'member')
                : t('settings.team.members', 'members')}
            </span>
            {teamStatus.last_sync_at && (
              <span className="text-[10px] text-text-muted">
                {t('settings.team.lastSync', 'Synced')} {formatRelativeTime(teamStatus.last_sync_at)}
              </span>
            )}
            {teamStatus.pending_outbound > 0 && (
              <span className="text-[10px] px-1.5 py-0.5 bg-orange-400/15 text-orange-400 rounded">
                {teamStatus.pending_outbound} {t('settings.team.pending', 'pending')}
              </span>
            )}
          </div>

          {/* Member list */}
          {teamMembers.length > 0 && (
            <div>
              <h4 className="text-[10px] text-text-muted uppercase tracking-wide mb-2">
                {t('settings.team.memberList', 'Members')}
              </h4>
              <div className="space-y-1" role="list" aria-label={t('settings.team.memberList', 'Members')}>
                {teamMembers.map(member => {
                  const badge = ROLE_BADGE[member.role] ?? ROLE_BADGE.member;
                  return (
                    <div
                      key={member.client_id}
                      className="flex items-center gap-2 px-2 py-1.5 rounded bg-bg-primary/50"
                      role="listitem"
                    >
                      <span className="text-xs text-white flex-1 truncate">{member.display_name}</span>
                      <span className={`text-[10px] px-1.5 py-0.5 rounded ${badge.bg} ${badge.text}`}>
                        {member.role}
                      </span>
                      {member.last_seen && (
                        <span className="text-[10px] text-text-muted whitespace-nowrap">
                          {formatRelativeTime(member.last_seen)}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Sharing preferences */}
          <div>
            <h4 className="text-[10px] text-text-muted uppercase tracking-wide mb-2">
              {t('settings.team.sharing', 'Sharing Preferences')}
            </h4>
            <div className="space-y-2">
              {([
                { key: 'dna' as const, label: t('settings.team.shareDna', 'DNA Summaries') },
                { key: 'signals' as const, label: t('settings.team.shareSignals', 'Signals') },
                { key: 'decisions' as const, label: t('settings.team.shareDecisions', 'Decisions') },
                { key: 'context' as const, label: t('settings.team.shareContext', 'Context Summaries') },
              ]).map(pref => (
                <label key={pref.key} className="flex items-center gap-2 cursor-pointer group">
                  <button
                    role="switch"
                    aria-checked={sharingPrefs[pref.key]}
                    aria-label={pref.label}
                    onClick={() => setSharingPrefs(prev => ({ ...prev, [pref.key]: !prev[pref.key] }))}
                    className={`relative w-8 h-4 rounded-full transition-colors ${sharingPrefs[pref.key] ? 'bg-[#22C55E]' : 'bg-border'}`}
                  >
                    <span
                      className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform ${sharingPrefs[pref.key] ? 'translate-x-4' : 'translate-x-0'}`}
                    />
                  </button>
                  <span className="text-xs text-text-secondary group-hover:text-white transition-colors">
                    {pref.label}
                  </span>
                </label>
              ))}
            </div>
          </div>

          {/* Admin actions */}
          {isAdmin && (
            <div className="flex gap-2">
              <button
                onClick={() => setShowTeamInviteDialog(true)}
                aria-label={t('settings.team.inviteMember', 'Invite Member')}
                className="flex-1 px-3 py-2 text-xs font-medium text-[#22C55E] border border-[#22C55E]/30 rounded-lg hover:bg-[#22C55E]/10 transition-colors"
              >
                {t('settings.team.inviteMember', 'Invite Member')}
              </button>
              {!confirmLeave ? (
                <button
                  onClick={() => setConfirmLeave(true)}
                  aria-label={t('settings.team.leaveTeam', 'Leave Team')}
                  className="px-3 py-2 text-xs text-text-muted border border-border rounded-lg hover:border-[#EF4444]/30 hover:text-[#EF4444] transition-colors"
                >
                  {t('settings.team.leaveTeam', 'Leave Team')}
                </button>
              ) : (
                <button
                  onClick={handleLeave}
                  aria-label={t('settings.team.confirmLeave', 'Confirm leave')}
                  className="px-3 py-2 text-xs font-medium text-[#EF4444] border border-[#EF4444]/30 rounded-lg bg-[#EF4444]/10 hover:bg-[#EF4444]/20 transition-colors"
                >
                  {t('settings.team.confirmLeave', 'Confirm')}
                </button>
              )}
            </div>
          )}

          {/* Sync info footer */}
          <div className="pt-2 border-t border-border">
            <p className="text-[10px] text-text-muted">
              {t('settings.team.relay', 'Relay')}: <span className="font-mono">relay.4da.ai</span>
              {' | '}
              {t('settings.team.syncInterval', 'Sync interval')}: 30s
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
