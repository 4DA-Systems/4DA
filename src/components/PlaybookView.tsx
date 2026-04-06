import { useEffect, useCallback, useMemo, useRef, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import type { InsightContent } from '../types/personalization';
import { SovereignProfile } from './playbook/SovereignProfile';
import { StreetHealthBadge } from './playbook/StreetHealthBadge';
import { TemplateLibrary } from './playbook/TemplateLibrary';
import { PlaybookSidebar } from './playbook/PlaybookSidebar';
import { useGameComponent } from '../hooks/use-game-component';
import PlaybookLessonList from './playbook/PlaybookLessonList';
import { PlaybookWisdomResonance } from './awe/PlaybookWisdomResonance';

function PlaybookPathway({ progress, streak }: { progress: number; streak: number }) {
  const { containerRef, elementRef } = useGameComponent('game-playbook-pathway');

  useEffect(() => {
    elementRef.current?.setParam?.('progress', progress);
    elementRef.current?.setParam?.('streak', streak);
    elementRef.current?.setParam?.('momentum', Math.max(0.3, progress));
  }, [progress, streak, elementRef]);

  return <div ref={containerRef} className="w-full h-10 rounded-lg overflow-hidden opacity-50" aria-hidden="true" />;
}

export const PlaybookView = memo(function PlaybookView() {
  const { t } = useTranslation();
  const {
    playbookModules,
    playbookContent,
    playbookProgress,
    playbookLoading,
    playbookError,
    activeModuleId,
    streetsTier,
    personalizedLessons,
  } = useAppStore(
    useShallow((s) => ({
      playbookModules: s.playbookModules,
      playbookContent: s.playbookContent,
      playbookProgress: s.playbookProgress,
      playbookLoading: s.playbookLoading,
      playbookError: s.playbookError,
      activeModuleId: s.activeModuleId,
      streetsTier: s.streetsTier,
      personalizedLessons: s.personalizedLessons,
    })),
  );

  const isBrowserMode = useAppStore((s) => s.isBrowserMode);
  const loadModules = useAppStore((s) => s.loadPlaybookModules);
  const loadContent = useAppStore((s) => s.loadPlaybookContent);
  const loadProgress = useAppStore((s) => s.loadPlaybookProgress);
  const markComplete = useAppStore((s) => s.markLessonComplete);
  const loadPersonalized = useAppStore((s) => s.loadPersonalizedContent);
  const loadStreetsTier = useAppStore((s) => s.loadStreetsTier);

  const [showTemplates, setShowTemplates] = useState(false);
  const [personalizeBannerOpen, setPersonalizeBannerOpen] = useState(true);

  // Load modules, progress, and streets tier on mount
  useEffect(() => {
    loadModules();
    loadProgress();
    loadStreetsTier();
  }, [loadModules, loadProgress, loadStreetsTier]);

  const handleModuleClick = useCallback(
    (moduleId: string) => {
      setShowTemplates(false);
      loadContent(moduleId);
    },
    [loadContent],
  );

  const handleShowTemplates = useCallback(() => {
    setShowTemplates(true);
  }, []);

  const addToast = useAppStore((s) => s.addToast);

  const handleLessonToggle = useCallback(
    (moduleId: string, lessonIdx: number) => {
      // Determine if this is marking complete (not toggling off)
      const progress = useAppStore.getState().playbookProgress;
      const mod = progress?.modules.find((m) => m.module_id === moduleId);
      const wasComplete = mod?.completed_lessons.includes(lessonIdx) ?? false;

      markComplete(moduleId, lessonIdx);

      // Show learning narrative toast when completing a lesson
      if (!wasComplete) {
        const content = useAppStore.getState().playbookContent;
        const lessonTitle = content?.lessons[lessonIdx]?.title;
        if (lessonTitle) {
          addToast(
            'success',
            t('streets:streets.lessonLearned', { title: lessonTitle }),
          );
        }
      }
    },
    [markComplete, addToast, t],
  );

  // Track already-requested personalization keys to avoid duplicate IPC calls
  const requestedKeysRef = useRef(new Set<string>());

  // Load personalized content for each lesson when module content changes
  useEffect(() => {
    if (!playbookContent) return;
    const moduleId = playbookContent.module_id;

    // Reset tracked keys when switching modules
    requestedKeysRef.current = new Set<string>();

    playbookContent.lessons.forEach((_, idx) => {
      const key = `${moduleId}:${idx}`;
      if (!personalizedLessons[key] && !requestedKeysRef.current.has(key)) {
        requestedKeysRef.current.add(key);
        loadPersonalized(moduleId, idx);
      }
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only fire on module content change, not on personalizedLessons updates
  }, [playbookContent, loadPersonalized]);

  // Listen for LLM hydration events and upgrade insight blocks in-place
  useEffect(() => {
    const unlisten = listen<{
      module_id: string;
      lesson_idx: number;
      block_id: string;
      content: InsightContent;
    }>('personalization-llm-upgrade', (event) => {
      const { module_id, lesson_idx, block_id, content } = event.payload;
      const key = `${module_id}:${lesson_idx}`;
      const current = useAppStore.getState().personalizedLessons;
      const lesson = current[key];
      if (!lesson) return;

      const updatedBlocks = lesson.insight_blocks.map((block) =>
        block.block_id === block_id ? { ...block, content } : block,
      );
      useAppStore.setState({
        personalizedLessons: { ...current, [key]: { ...lesson, insight_blocks: updatedBlocks } },
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Build a set of completed lesson indices for the active module
  const completedSet = useMemo(() => {
    if (!playbookProgress || !activeModuleId) return new Set<number>();
    const mod = playbookProgress.modules.find((m) => m.module_id === activeModuleId);
    return new Set(mod?.completed_lessons ?? []);
  }, [playbookProgress, activeModuleId]);

  return (
    <div className="flex gap-6 min-h-[600px]">
      {/* Sidebar - Module Navigation */}
      <PlaybookSidebar
        playbookModules={playbookModules}
        playbookProgress={playbookProgress}
        activeModuleId={activeModuleId}
        streetsTier={streetsTier}
        showTemplates={showTemplates}
        onModuleClick={handleModuleClick}
        onShowTemplates={handleShowTemplates}
      />

      {/* Content Area */}
      <main className="flex-1 min-w-0">
        <StreetHealthBadge />

        {playbookError && (
          <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
            <p className="text-text-secondary text-sm">
              {isBrowserMode
                ? t('error.playbookBrowser')
                : t('error.playbookFailed')}
            </p>
            {!isBrowserMode && (
              <button
                onClick={() => {
                  loadModules();
                  loadProgress();
                  loadStreetsTier();
                }}
                aria-label={t('action.retry')}
                className="px-3 py-1.5 text-xs bg-bg-tertiary hover:bg-white/10 rounded transition-colors text-text-secondary"
              >
                {t('action.retry')}
              </button>
            )}
          </div>
        )}

        {showTemplates && (
          <div className="bg-bg-secondary border border-border rounded-xl p-6">
            <TemplateLibrary />
          </div>
        )}

        {!showTemplates && !activeModuleId && !playbookLoading && !playbookError && (
          <div className="flex flex-col items-center justify-center h-96 text-center">
            <div className="w-16 h-16 bg-accent-gold/10 rounded-2xl flex items-center justify-center mb-4">
              <span className="text-2xl text-accent-gold font-bold">S</span>
            </div>
            <h3 className="text-lg font-semibold text-white mb-2">{t('streets:streets.title')}</h3>
            <p className="text-sm text-text-secondary max-w-md mb-4">
              {t('streets:streets.selectModuleDescription')}{' '}
              {t('streets:streets.selectModule')}
            </p>
            <button
              onClick={() => handleModuleClick('S')}
              aria-label={t('streets:streets.startWith')}
              className="px-4 py-2 bg-accent-gold text-black text-sm font-medium rounded-lg hover:bg-[#C4A030] transition-colors"
            >
              {t('streets:streets.startWith')}
            </button>
          </div>
        )}

        {!showTemplates && playbookLoading && (
          <div className="space-y-4">
            <div className="bg-bg-secondary border border-border rounded-xl p-6">
              <div className="h-5 w-48 bg-bg-tertiary rounded animate-pulse mb-3" />
              <div className="h-4 w-72 bg-bg-tertiary rounded animate-pulse" />
            </div>
            {Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="bg-bg-secondary border border-border rounded-xl p-6">
                <div className="h-4 w-40 bg-bg-tertiary rounded animate-pulse mb-3" />
                <div className="space-y-2">
                  <div className="h-3 bg-bg-tertiary rounded animate-pulse" style={{ width: '90%' }} />
                  <div className="h-3 bg-bg-tertiary rounded animate-pulse" style={{ width: '75%' }} />
                  <div className="h-3 bg-bg-tertiary rounded animate-pulse" style={{ width: '60%' }} />
                </div>
              </div>
            ))}
          </div>
        )}

        {!showTemplates && playbookContent && !playbookLoading && (
          <div className="space-y-4">
            <PlaybookPathway
              progress={playbookContent.lessons.length > 0 ? completedSet.size / playbookContent.lessons.length : 0}
              streak={Object.keys(personalizedLessons).length > 0 ? 0.7 : 0.3}
            />

            {/* Module Header */}
            <div className="bg-bg-secondary border border-border rounded-xl p-6">
              <div className="flex items-center gap-3 mb-2">
                <span className="px-2 py-1 bg-accent-gold/20 text-accent-gold text-xs font-bold rounded">
                  {playbookContent.module_id}
                </span>
              </div>
              <h2 className="text-xl font-semibold text-white">{playbookContent.title}</h2>
              <p className="text-sm text-text-secondary mt-1">{playbookContent.description}</p>
            </div>

            {/* Personalization banner */}
            {Object.keys(personalizedLessons).some((k) => k.startsWith(playbookContent.module_id + ':')) && (
              <div className="border border-accent-gold/15 rounded-xl bg-accent-gold/5 overflow-hidden">
                <button
                  onClick={() => setPersonalizeBannerOpen((p) => !p)}
                  className="w-full flex items-center gap-2 px-4 py-2.5 text-start"
                  aria-expanded={personalizeBannerOpen}
                >
                  <span className="text-[10px] text-accent-gold uppercase tracking-wider font-semibold">
                    {t('playbook.personalizedBanner')}
                  </span>
                  <span className={`ms-auto text-[10px] text-accent-gold/60 transition-transform duration-200 ${personalizeBannerOpen ? 'rotate-90' : ''}`} aria-hidden="true">{'\u25B8'}</span>
                </button>
                {personalizeBannerOpen && (
                  <div className="px-4 pb-3">
                    <p className="text-[11px] text-text-secondary leading-relaxed">
                      {t('playbook.personalizedBannerDesc')}
                    </p>
                  </div>
                )}
              </div>
            )}

            {/* Lessons */}
            <PlaybookLessonList
              moduleId={playbookContent.module_id}
              lessons={playbookContent.lessons}
              completedSet={completedSet}
              personalizedLessons={personalizedLessons}
              onLessonToggle={handleLessonToggle}
            />

            {/* AWE Wisdom Resonance — shows which lessons align with real decisions */}
            <PlaybookWisdomResonance
              moduleTopics={playbookContent.lessons.map(l => l.title)}
            />

            {/* Sovereign Profile Panel — show when viewing Module S */}
            {playbookContent.module_id === 'S' && (
              <SovereignProfile
                onGenerateDocument={() => {
                  /* document rendered inline by SovereignProfile */
                }}
              />
            )}

          </div>
        )}
      </main>
    </div>
  );
});
