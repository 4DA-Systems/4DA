// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

export type FormMode = 'none' | 'create' | 'join';

interface TeamFormsProps {
  formMode: FormMode;
  setFormMode: (mode: FormMode) => void;
  relayUrl: string;
  setRelayUrl: (url: string) => void;
  displayName: string;
  setDisplayName: (name: string) => void;
  inviteCode: string;
  setInviteCode: (code: string) => void;
  teamLoading: boolean;
  onCreate: () => void;
  onJoin: () => void;
}

export function TeamForms({
  formMode,
  setFormMode,
  relayUrl,
  setRelayUrl,
  displayName,
  setDisplayName,
  inviteCode,
  setInviteCode,
  teamLoading,
  onCreate,
  onJoin,
}: TeamFormsProps) {
  const { t } = useTranslation();

  if (formMode === 'none') {
    return (
      <div className="flex gap-2">
        <button
          onClick={() => setFormMode('create')}
          aria-label={t('settings.team.createTeam', 'Create Team')}
          className="flex-1 px-4 py-2 text-xs font-medium text-bg-primary bg-success rounded-lg hover:opacity-90 transition-colors"
        >
          {t('settings.team.createTeam', 'Create Team')}
        </button>
        <button
          onClick={() => setFormMode('join')}
          aria-label={t('settings.team.joinTeam', 'Join Team')}
          className="flex-1 px-4 py-2 text-xs font-medium text-text-secondary border border-border rounded-lg hover:border-success/50 hover:text-text-primary transition-colors"
        >
          {t('settings.team.joinTeam', 'Join Team')}
        </button>
      </div>
    );
  }

  if (formMode === 'create') {
    return (
      <div className="space-y-3">
        <label className="block">
          <span className="text-[10px] text-text-muted uppercase tracking-wide">
            {t('settings.team.relayUrl', 'Relay URL')}
          </span>
          <input
            type="url"
            value={relayUrl}
            onChange={(e) => setRelayUrl(e.target.value)}
            placeholder="https://relay.4da.ai"
            className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-text-primary placeholder-gray-600 focus:outline-none focus:border-success/50 font-mono"
          />
        </label>
        <label className="block">
          <span className="text-[10px] text-text-muted uppercase tracking-wide">
            {t('settings.team.displayName', 'Display Name')}
          </span>
          <input
            type="text"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            placeholder="Your name"
            className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-text-primary placeholder-gray-600 focus:outline-none focus:border-success/50"
          />
        </label>
        <div className="flex gap-2">
          <button
            onClick={onCreate}
            disabled={
              teamLoading || !displayName.trim() || !relayUrl.trim()
            }
            className="flex-1 px-4 py-2 text-xs font-medium text-bg-primary bg-success rounded-lg hover:opacity-90 transition-colors disabled:opacity-50"
          >
            {teamLoading
              ? '...'
              : t('settings.team.createTeam', 'Create Team')}
          </button>
          <button
            onClick={() => setFormMode('none')}
            className="px-4 py-2 text-xs text-text-muted hover:text-text-primary transition-colors"
          >
            {t('action.cancel', 'Cancel')}
          </button>
        </div>
      </div>
    );
  }

  // formMode === 'join'
  return (
    <div className="space-y-3">
      <label className="block">
        <span className="text-[10px] text-text-muted uppercase tracking-wide">
          {t('settings.team.relayUrl', 'Relay URL')}
        </span>
        <input
          type="url"
          value={relayUrl}
          onChange={(e) => setRelayUrl(e.target.value)}
          placeholder="https://relay.4da.ai"
          className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-text-primary placeholder-gray-600 focus:outline-none focus:border-success/50 font-mono"
        />
      </label>
      <label className="block">
        <span className="text-[10px] text-text-muted uppercase tracking-wide">
          {t('settings.team.inviteCode', 'Invite Code')}
        </span>
        <input
          type="text"
          value={inviteCode}
          onChange={(e) => setInviteCode(e.target.value.toUpperCase().slice(0, 6))}
          placeholder="ABC123"
          maxLength={6}
          className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-text-primary placeholder-gray-600 focus:outline-none focus:border-success/50 font-mono tracking-widest"
        />
      </label>
      <label className="block">
        <span className="text-[10px] text-text-muted uppercase tracking-wide">
          {t('settings.team.displayName', 'Display Name')}
        </span>
        <input
          type="text"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder="Your name"
          className="mt-1 w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-xs text-text-primary placeholder-gray-600 focus:outline-none focus:border-success/50"
        />
      </label>
      <div className="flex gap-2">
        <button
          onClick={onJoin}
          disabled={
            teamLoading ||
            !displayName.trim() ||
            !relayUrl.trim() ||
            !inviteCode.trim()
          }
          className="flex-1 px-4 py-2 text-xs font-medium text-bg-primary bg-success rounded-lg hover:opacity-90 transition-colors disabled:opacity-50"
        >
          {teamLoading ? '...' : t('settings.team.joinTeam', 'Join Team')}
        </button>
        <button
          onClick={() => setFormMode('none')}
          className="px-4 py-2 text-xs text-text-muted hover:text-text-primary transition-colors"
        >
          {t('action.cancel', 'Cancel')}
        </button>
      </div>
    </div>
  );
}
