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
import { TemplateLibrary } from './TemplateLibrary';
import { VideoCurriculum } from './VideoCurriculum';
import { SUB_TAB_IDS, SUB_TAB_KEYS, TierBadge, SessionTypeBadge, StreetsGate, NewSessionDropdown } from './CoachViewParts';
import type { CoachSessionType } from '../../types/coach';

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
      case 'templates':
        return <TemplateLibrary />;
      case 'curriculum':
        return <VideoCurriculum />;
      default:
        return null;
    }
  }

  return (
    <div className="relative flex flex-col h-full min-h-[600px]">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-lg font-semibold text-white tracking-wide">{t('coach:coach.title')}</h1>
        <div className="flex items-center gap-2">
          {streetsTier === 'playbook' && (
            <span className={`text-xs px-2 py-0.5 rounded ${
              coachSessions.length >= 2
                ? 'bg-amber-500/10 text-amber-400'
                : 'text-text-muted'
            }`}>
              {t('coach.freeSession', { current: coachSessions.length, max: 2 })}
            </span>
          )}
          <TierBadge tier={streetsTier} />
        </div>
      </div>

      {/* Two-panel layout */}
      <div className="flex gap-4 flex-1 min-h-0">
        {/* Left sidebar */}
        <aside className="w-52 flex-shrink-0 bg-bg-secondary border border-border rounded-xl p-3 flex flex-col">
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
                      : 'hover:bg-bg-tertiary border border-transparent'
                  }`}
                >
                  <p className={`text-sm truncate ${isActive ? 'text-white font-medium' : 'text-text-secondary'}`}>
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
          <div className="mt-3 pt-3 border-t border-border space-y-0.5">
            {SUB_TAB_IDS.map(tabId => (
              <button
                key={tabId}
                onClick={() => setActiveTab(tabId)}
                className={`w-full text-left px-3 py-1.5 text-sm rounded-md transition-colors ${
                  activeTab === tabId
                    ? 'text-[#D4AF37] bg-[#D4AF37]/10 font-medium'
                    : 'text-text-secondary hover:text-white hover:bg-bg-tertiary'
                }`}
              >
                {t(SUB_TAB_KEYS[tabId])}
              </button>
            ))}
          </div>
        </aside>

        {/* Content area */}
        <main className="flex-1 min-w-0 bg-bg-secondary border border-border rounded-xl p-4 overflow-hidden flex flex-col">
          {renderContent()}
        </main>
      </div>

      {/* StreetsGate overlay for playbook-only tier (after 2 free sessions) */}
      {streetsTier === 'playbook' && coachSessions.length >= 2 && <StreetsGate />}
    </div>
  );
}
