import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const STATUS_COLORS: Record<string, string> = {
  proposed: 'bg-[#D4AF37]/15 text-[#D4AF37]',
  accepted: 'bg-[#22C55E]/15 text-[#22C55E]',
  rejected: 'bg-[#EF4444]/15 text-[#EF4444]',
};

const STANCE_COLORS: Record<string, string> = {
  for: 'text-[#22C55E]',
  against: 'text-[#EF4444]',
  abstain: 'text-text-muted',
};

export function TeamDecisionTracker() {
  const { t } = useTranslation();
  const teamDecisions = useAppStore(s => s.teamDecisions);
  const selectedDecision = useAppStore(s => s.selectedDecision);
  const decisionsLoading = useAppStore(s => s.decisionsLoading);
  const loadTeamDecisions = useAppStore(s => s.loadTeamDecisions);
  const loadDecisionDetail = useAppStore(s => s.loadDecisionDetail);
  const voteOnDecision = useAppStore(s => s.voteOnDecision);
  const resolveDecision = useAppStore(s => s.resolveDecision);

  const [statusFilter, setStatusFilter] = useState<string>('');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [voteStance, setVoteStance] = useState<string>('for');
  const [voteRationale, setVoteRationale] = useState('');
  const [showVoteForm, setShowVoteForm] = useState(false);

  useEffect(() => {
    loadTeamDecisions(statusFilter || undefined);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [statusFilter]);

  const handleExpand = (id: string) => {
    if (expandedId === id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(id);
    loadDecisionDetail(id);
    setShowVoteForm(false);
  };

  const handleVote = async () => {
    if (!expandedId) return;
    await voteOnDecision(expandedId, voteStance, voteRationale);
    setVoteRationale('');
    setShowVoteForm(false);
  };

  const handleResolve = async (id: string, status: string) => {
    await resolveDecision(id, status);
    loadTeamDecisions(statusFilter || undefined);
  };

  if (decisionsLoading && teamDecisions.length === 0) {
    return (
      <div className="flex items-center justify-center py-6">
        <span className="text-xs text-text-muted">{t('action.loading', 'Loading...')}</span>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Filter Bar */}
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-medium text-text-secondary">
          {t('team.decisions.title', 'Team Decisions')} ({teamDecisions.length})
        </h4>
        <select
          value={statusFilter}
          onChange={e => setStatusFilter(e.target.value)}
          className="px-2 py-1 text-[10px] bg-bg-tertiary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
          aria-label="Filter by status"
        >
          <option value="">{t('team.decisions.all', 'All')}</option>
          <option value="proposed">{t('team.decisions.proposed', 'Proposed')}</option>
          <option value="accepted">{t('team.decisions.accepted', 'Accepted')}</option>
          <option value="rejected">{t('team.decisions.rejected', 'Rejected')}</option>
        </select>
      </div>

      {teamDecisions.length === 0 ? (
        <p className="text-xs text-text-muted text-center py-6">
          {t('team.decisions.empty', 'No team decisions yet. Propose decisions from the Decision Log.')}
        </p>
      ) : (
        <div className="space-y-2">
          {teamDecisions.map(decision => {
            const isExpanded = expandedId === decision.id;
            const detail = isExpanded ? selectedDecision : null;

            return (
              <div
                key={decision.id}
                className="bg-bg-primary rounded-lg border border-border/50 overflow-hidden"
              >
                {/* Decision Header */}
                <button
                  onClick={() => handleExpand(decision.id)}
                  className="w-full px-3 py-2.5 flex items-center justify-between hover:bg-bg-tertiary/30 transition-colors text-left"
                  aria-expanded={isExpanded}
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-white font-medium truncate">{decision.title}</span>
                      <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${STATUS_COLORS[decision.status] || 'bg-border text-text-muted'}`}>
                        {decision.status}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-[10px] text-text-muted mt-0.5">
                      <span>{decision.decision_type}</span>
                      <span>&#183;</span>
                      <span>{decision.vote_count} {t('team.decisions.votes', 'votes')}</span>
                      <span>&#183;</span>
                      <span>{formatRelativeTime(decision.created_at)}</span>
                    </div>
                  </div>
                  <span className={`text-text-muted text-xs transition-transform ${isExpanded ? 'rotate-180' : ''}`}>
                    &#9660;
                  </span>
                </button>

                {/* Expanded Detail */}
                {isExpanded && (
                  <div className="border-t border-border/50 px-3 py-3 space-y-3">
                    {/* Rationale */}
                    <div>
                      <p className="text-[10px] text-text-muted mb-0.5">{t('team.decisions.rationale', 'Rationale')}</p>
                      <p className="text-xs text-text-secondary">{decision.rationale}</p>
                    </div>

                    {/* Votes */}
                    {detail?.votes && detail.votes.length > 0 && (
                      <div>
                        <p className="text-[10px] text-text-muted mb-1">{t('team.decisions.voteBreakdown', 'Votes')}</p>
                        <div className="space-y-1">
                          {detail.votes.map(vote => (
                            <div key={vote.voter_id} className="flex items-center gap-2 text-[10px]">
                              <span className={`font-medium ${STANCE_COLORS[vote.stance] || 'text-text-muted'}`}>
                                {vote.stance}
                              </span>
                              <span className="text-text-muted font-mono">{vote.voter_id.slice(0, 8)}</span>
                              {vote.rationale && (
                                <span className="text-text-secondary truncate">&mdash; {vote.rationale}</span>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Vote Form (only for proposed decisions) */}
                    {decision.status === 'proposed' && (
                      <div>
                        {showVoteForm ? (
                          <div className="space-y-2 bg-bg-tertiary rounded-lg p-2.5">
                            <div className="flex items-center gap-2">
                              {(['for', 'against', 'abstain'] as const).map(stance => (
                                <button
                                  key={stance}
                                  onClick={() => setVoteStance(stance)}
                                  className={`text-[10px] px-2 py-1 rounded border transition-colors ${
                                    voteStance === stance
                                      ? `${STANCE_COLORS[stance]} border-current bg-white/5`
                                      : 'text-text-muted border-border hover:border-text-muted'
                                  }`}
                                >
                                  {stance}
                                </button>
                              ))}
                            </div>
                            <input
                              type="text"
                              value={voteRationale}
                              onChange={e => setVoteRationale(e.target.value)}
                              placeholder={t('team.decisions.voteRationale', 'Why? (optional)')}
                              className="w-full px-2 py-1.5 text-[10px] bg-bg-primary border border-border rounded text-white focus:outline-none focus:border-[#22C55E]/50"
                              onKeyDown={e => e.key === 'Enter' && handleVote()}
                            />
                            <div className="flex items-center gap-2">
                              <button
                                onClick={handleVote}
                                className="text-[10px] px-3 py-1 bg-[#22C55E]/15 text-[#22C55E] rounded hover:bg-[#22C55E]/25 transition-colors"
                              >
                                {t('team.decisions.submitVote', 'Submit Vote')}
                              </button>
                              <button
                                onClick={() => setShowVoteForm(false)}
                                className="text-[10px] text-text-muted hover:text-white transition-colors"
                              >
                                {t('action.cancel', 'Cancel')}
                              </button>
                            </div>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => setShowVoteForm(true)}
                              className="text-[10px] px-2.5 py-1 bg-[#22C55E]/10 text-[#22C55E] rounded hover:bg-[#22C55E]/20 transition-colors"
                            >
                              {t('team.decisions.castVote', 'Cast Vote')}
                            </button>
                            <button
                              onClick={() => handleResolve(decision.id, 'accepted')}
                              className="text-[10px] px-2.5 py-1 text-text-muted hover:text-[#22C55E] transition-colors"
                            >
                              {t('team.decisions.accept', 'Accept')}
                            </button>
                            <button
                              onClick={() => handleResolve(decision.id, 'rejected')}
                              className="text-[10px] px-2.5 py-1 text-text-muted hover:text-[#EF4444] transition-colors"
                            >
                              {t('team.decisions.reject', 'Reject')}
                            </button>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
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
