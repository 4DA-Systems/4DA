import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

export function TeamMemberList() {
  const { t } = useTranslation();
  const teamMembers = useAppStore(s => s.teamMembers);
  const teamStatus = useAppStore(s => s.teamStatus);

  if (teamMembers.length === 0) {
    return (
      <p className="text-xs text-text-muted text-center py-4">
        {t('team.members.empty', 'No team members yet. Invite someone to get started.')}
      </p>
    );
  }

  return (
    <div className="space-y-1.5">
      {teamMembers.map(member => {
        const isYou = member.client_id === teamStatus?.client_id;
        const isOnline = member.last_seen
          ? (Date.now() - new Date(member.last_seen).getTime()) < 5 * 60 * 1000
          : false;

        return (
          <div
            key={member.client_id}
            className="flex items-center justify-between px-2.5 py-2 rounded-lg hover:bg-bg-tertiary/50 transition-colors"
          >
            <div className="flex items-center gap-2.5">
              {/* Avatar */}
              <div className={`w-6 h-6 rounded-full flex items-center justify-center text-[10px] font-semibold ${
                member.role === 'admin'
                  ? 'bg-[#22C55E]/15 text-[#22C55E]'
                  : 'bg-[#3B82F6]/15 text-[#3B82F6]'
              }`}>
                {member.display_name.charAt(0).toUpperCase()}
              </div>

              {/* Name + You badge */}
              <div>
                <div className="flex items-center gap-1.5">
                  <span className="text-xs text-white">{member.display_name}</span>
                  {isYou && (
                    <span className="text-[9px] px-1 py-0.5 bg-border text-text-muted rounded">
                      {t('team.members.you', 'you')}
                    </span>
                  )}
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2">
              {/* Role badge */}
              <span className={`text-[10px] px-1.5 py-0.5 rounded ${
                member.role === 'admin'
                  ? 'bg-[#22C55E]/10 text-[#22C55E]'
                  : 'bg-[#3B82F6]/10 text-[#3B82F6]'
              }`}>
                {member.role}
              </span>

              {/* Presence */}
              <div className="flex items-center gap-1">
                <div className={`w-1.5 h-1.5 rounded-full ${
                  isOnline ? 'bg-[#22C55E]' : 'bg-text-muted'
                }`} />
                {member.last_seen && !isOnline && (
                  <span className="text-[9px] text-text-muted" title={member.last_seen}>
                    {formatRelativeTime(member.last_seen)}
                  </span>
                )}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'now';
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  const days = Math.floor(hrs / 24);
  return `${days}d`;
}
