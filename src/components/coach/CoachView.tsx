import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { getRelativeTime } from '../../utils/briefing-parser';
import { CoachChat } from './CoachChat';
import { EngineRecommender } from './EngineRecommender';
import { StrategyViewer } from './StrategyViewer';
import { LaunchReviewForm } from './LaunchReviewForm';
import { ProgressDashboard } from './ProgressDashboard';
import type { CoachSessionType, StreetsTier } from '../../types/coach';

// Placeholder for sub-components not yet implemented
const Placeholder = ({ name }: { name: string }) => {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-center h-64 text-[#666]">
      <p>{t('coach:coach.comingSoon', { name })}</p>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Sub-tab definitions
// ---------------------------------------------------------------------------

const SUB_TAB_IDS: CoachSessionType[] = ['chat', 'engine_recommender', 'strategy', 'launch_review', 'progress'];

const SUB_TAB_KEYS: Record<CoachSessionType, string> = {
  chat: 'coach:coach.tab.chat',
  engine_recommender: 'coach:coach.tab.engines',
  strategy: 'coach:coach.tab.strategy',
  launch_review: 'coach:coach.tab.launchReview',
  progress: 'coach:coach.tab.progress',
};

// ---------------------------------------------------------------------------
// Tier badge
// ---------------------------------------------------------------------------

function TierBadge({ tier }: { tier: StreetsTier }) {
  const { t } = useTranslation();
  const config: Record<StreetsTier, { labelKey: string; color: string }> = {
    playbook: { labelKey: 'coach:coach.tier.playbook', color: 'bg-[#1F1F1F] text-[#A0A0A0] border-[#2A2A2A]' },
    community: { labelKey: 'coach:coach.tier.community', color: 'bg-[#D4AF37]/15 text-[#D4AF37] border-[#D4AF37]/30' },
    cohort: { labelKey: 'coach:coach.tier.cohort', color: 'bg-[#22C55E]/15 text-[#22C55E] border-[#22C55E]/30' },
  };
  const { labelKey, color } = config[tier];

  return (
    <span className={`px-2.5 py-1 text-xs font-medium rounded-md border ${color}`}>
      {t(labelKey)}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Session type badge (small, inline)
// ---------------------------------------------------------------------------

function SessionTypeBadge({ type }: { type: string }) {
  const { t } = useTranslation();
  const shortLabelKeys: Record<string, string> = {
    chat: 'coach:coach.sessionType.chat',
    engine_recommender: 'coach:coach.sessionType.engine',
    strategy: 'coach:coach.sessionType.strategy',
    launch_review: 'coach:coach.sessionType.launch',
    progress: 'coach:coach.sessionType.progress',
  };

  return (
    <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-[#1F1F1F] text-[#666] border border-[#2A2A2A]">
      {shortLabelKeys[type] ? t(shortLabelKeys[type]) : type}
    </span>
  );
}

// ---------------------------------------------------------------------------
// StreetsGate overlay (shown when tier is 'playbook')
// ---------------------------------------------------------------------------

function StreetsGate() {
  const { t } = useTranslation();
  const activateStreetsLicense = useAppStore(s => s.activateStreetsLicense);
  const [key, setKey] = useState('');
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleActivate = async () => {
    if (!key.trim()) return;
    setActivating(true);
    setError(null);
    const ok = await activateStreetsLicense(key.trim());
    setActivating(false);
    if (!ok) setError(t('coach:coach.gate.invalidKey'));
  };

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-[#0A0A0A]/80 backdrop-blur-sm rounded-xl">
      <div className="bg-[#141414] border border-[#D4AF37]/30 rounded-xl px-8 py-6 text-center max-w-sm shadow-lg">
        <div className="flex items-center justify-center gap-2 mb-3">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-[#D4AF37]">
            <path d="M8 1L10 6H15L11 9.5L12.5 15L8 11.5L3.5 15L5 9.5L1 6H6L8 1Z" fill="currentColor" />
          </svg>
          <span className="text-sm font-semibold text-[#D4AF37] tracking-wide uppercase">
            {t('coach:coach.gate.title')}
          </span>
        </div>
        <p className="text-sm text-[#A0A0A0] mb-1">
          {t('coach:coach.gate.requiresLicense')}
        </p>
        <p className="text-xs text-[#666] mb-4">
          {t('coach:coach.gate.freeModules')}
        </p>
        <div className="flex flex-col gap-2">
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block px-5 py-2.5 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors"
          >
            {t('coach:coach.gate.getCommunity')}
          </a>
          <div className="flex gap-2">
            <input
              type="text"
              value={key}
              onChange={e => setKey(e.target.value)}
              placeholder={t('coach:coach.gate.enterKey')}
              onKeyDown={e => e.key === 'Enter' && handleActivate()}
              className="flex-1 px-3 py-2 bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder-[#666] focus:outline-none focus:border-[#D4AF37]/50"
            />
            <button
              onClick={handleActivate}
              disabled={activating || !key.trim()}
              className="px-3 py-2 text-sm font-medium bg-[#1F1F1F] text-[#A0A0A0] border border-[#2A2A2A] rounded-lg hover:bg-[#2A2A2A] hover:text-white transition-colors disabled:opacity-50"
            >
              {activating ? '...' : t('action.activate')}
            </button>
          </div>
          {error && <p className="text-xs text-[#EF4444]">{error}</p>}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// New Session dropdown
// ---------------------------------------------------------------------------

function NewSessionDropdown({
  onSelect,
}: {
  onSelect: (type: CoachSessionType) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(prev => !prev)}
        className="w-full flex items-center justify-center gap-1.5 px-3 py-2 text-sm font-medium text-white bg-[#1F1F1F] border border-[#2A2A2A] rounded-lg hover:border-[#D4AF37]/40 transition-colors"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        {t('coach:coach.newSession')}
      </button>

      {open && (
        <>
          {/* Backdrop to close dropdown */}
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute left-0 right-0 mt-1 z-20 bg-[#141414] border border-[#2A2A2A] rounded-lg shadow-lg overflow-hidden">
            {SUB_TAB_IDS.map(tabId => (
              <button
                key={tabId}
                onClick={() => {
                  onSelect(tabId);
                  setOpen(false);
                }}
                className="w-full text-left px-3 py-2 text-sm text-[#A0A0A0] hover:bg-[#1F1F1F] hover:text-white transition-colors"
              >
                {t(SUB_TAB_KEYS[tabId])}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

// ===========================================================================
// CoachView (main export)
// ===========================================================================

export function CoachView() {
  const { t } = useTranslation();
  const {
    streetsTier,
    coachSessions,
    activeSessionId,
  } = useAppStore(
    useShallow(s => ({
      streetsTier: s.streetsTier,
      coachSessions: s.coachSessions,
      activeSessionId: s.activeSessionId,
    })),
  );

  const loadStreetsTier = useAppStore(s => s.loadStreetsTier);
  const loadCoachSessions = useAppStore(s => s.loadCoachSessions);
  const loadCoachNudges = useAppStore(s => s.loadCoachNudges);
  const createCoachSession = useAppStore(s => s.createCoachSession);
  const deleteCoachSession = useAppStore(s => s.deleteCoachSession);
  const setActiveSession = useAppStore(s => s.setActiveSession);

  const [activeTab, setActiveTab] = useState<CoachSessionType>('chat');
  const [hoveredSessionId, setHoveredSessionId] = useState<string | null>(null);

  // Load initial data
  useEffect(() => {
    loadStreetsTier();
    loadCoachSessions();
    loadCoachNudges();
  }, [loadStreetsTier, loadCoachSessions, loadCoachNudges]);

  const handleNewSession = useCallback(async (type: CoachSessionType) => {
    await createCoachSession(type);
    setActiveTab(type);
  }, [createCoachSession]);

  const handleDeleteSession = useCallback(async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    await deleteCoachSession(id);
  }, [deleteCoachSession]);

  const handleSelectSession = useCallback((id: string, type: string) => {
    setActiveSession(id);
    // Map the session type to a valid tab
    const validTypes: CoachSessionType[] = ['chat', 'engine_recommender', 'strategy', 'launch_review', 'progress'];
    if (validTypes.includes(type as CoachSessionType)) {
      setActiveTab(type as CoachSessionType);
    }
  }, [setActiveSession]);

  // Render the active content view
  function renderContent() {
    switch (activeTab) {
      case 'chat':
        return <CoachChat />;
      case 'engine_recommender':
        return <EngineRecommender />;
      case 'strategy':
        return <StrategyViewer />;
      case 'launch_review':
        return <LaunchReviewForm />;
      case 'progress':
        return <ProgressDashboard />;
      default:
        return <Placeholder name="Unknown view" />;
    }
  }

  return (
    <div className="relative flex flex-col h-full min-h-[600px]">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-lg font-semibold text-white tracking-wide">{t('coach:coach.title')}</h1>
        <TierBadge tier={streetsTier} />
      </div>

      {/* Two-panel layout */}
      <div className="flex gap-4 flex-1 min-h-0">
        {/* Left sidebar */}
        <aside className="w-52 flex-shrink-0 bg-[#141414] border border-[#2A2A2A] rounded-xl p-3 flex flex-col">
          {/* New Session button */}
          <div className="mb-3">
            <NewSessionDropdown onSelect={handleNewSession} />
          </div>

          {/* Session list */}
          <div className="flex-1 overflow-y-auto space-y-1 min-h-0">
            {coachSessions.length === 0 && (
              <p className="text-xs text-[#666] text-center py-4">
                {t('coach:coach.noSessions')}
              </p>
            )}
            {coachSessions.map(session => {
              const isActive = session.id === activeSessionId;
              const isHovered = session.id === hoveredSessionId;

              return (
                <button
                  key={session.id}
                  onClick={() => handleSelectSession(session.id, session.session_type)}
                  onMouseEnter={() => setHoveredSessionId(session.id)}
                  onMouseLeave={() => setHoveredSessionId(null)}
                  className={`w-full text-left px-3 py-2 rounded-lg transition-all group relative ${
                    isActive
                      ? 'bg-[#D4AF37]/10 border border-[#D4AF37]/30'
                      : 'hover:bg-[#1F1F1F] border border-transparent'
                  }`}
                >
                  <p className={`text-sm truncate ${isActive ? 'text-white font-medium' : 'text-[#A0A0A0]'}`}>
                    {session.title}
                  </p>
                  <div className="flex items-center gap-1.5 mt-0.5">
                    <SessionTypeBadge type={session.session_type} />
                    <span className="text-[10px] text-[#666]">
                      {getRelativeTime(new Date(session.updated_at))}
                    </span>
                  </div>

                  {/* Delete button on hover */}
                  {(isHovered || isActive) && (
                    <button
                      onClick={(e) => handleDeleteSession(e, session.id)}
                      className="absolute top-2 right-2 w-5 h-5 flex items-center justify-center rounded text-[#666] hover:text-[#EF4444] hover:bg-[#EF4444]/10 transition-colors"
                      title={t('coach:coach.deleteSession')}
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <line x1="18" y1="6" x2="6" y2="18" />
                        <line x1="6" y1="6" x2="18" y2="18" />
                      </svg>
                    </button>
                  )}
                </button>
              );
            })}
          </div>

          {/* Sub-tab navigation */}
          <div className="mt-3 pt-3 border-t border-[#2A2A2A] space-y-0.5">
            {SUB_TAB_IDS.map(tabId => (
              <button
                key={tabId}
                onClick={() => setActiveTab(tabId)}
                className={`w-full text-left px-3 py-1.5 text-sm rounded-md transition-colors ${
                  activeTab === tabId
                    ? 'text-[#D4AF37] bg-[#D4AF37]/10 font-medium'
                    : 'text-[#A0A0A0] hover:text-white hover:bg-[#1F1F1F]'
                }`}
              >
                {t(SUB_TAB_KEYS[tabId])}
              </button>
            ))}
          </div>
        </aside>

        {/* Content area */}
        <main className="flex-1 min-w-0 bg-[#141414] border border-[#2A2A2A] rounded-xl p-4 overflow-hidden flex flex-col">
          {renderContent()}
        </main>
      </div>

      {/* StreetsGate overlay for playbook-only tier */}
      {streetsTier === 'playbook' && <StreetsGate />}
    </div>
  );
}
