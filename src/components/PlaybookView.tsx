import { useEffect, useCallback, useMemo, useRef, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { renderMarkdown } from '../utils/playbook-markdown';
import type { InsightContent } from '../types/personalization';
import { SovereignProfile } from './playbook/SovereignProfile';
import { StreetHealthBadge } from './playbook/StreetHealthBadge';
import { SovereignInsightCard } from './playbook/SovereignInsightCard';
import { SovereignConnectionBlock } from './playbook/SovereignConnectionBlock';
import { DiffRibbon } from './playbook/DiffRibbon';
import { FeedEchoBlock } from './playbook/FeedEchoBlock';
import { ProgressiveRevealBanner } from './playbook/ProgressiveRevealBanner';
import { PersonalizationDepthIndicator } from './playbook/PersonalizationDepthIndicator';
import { TemplateLibrary } from './playbook/TemplateLibrary';
import { PlaybookSidebar } from './playbook/PlaybookSidebar';
import { registerGameComponent } from '../lib/game-components';

function PlaybookPathway({ progress, streak }: { progress: number; streak: number }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const elementRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    registerGameComponent('game-playbook-pathway').then(() => {
      if (!containerRef.current || elementRef.current) return;
      const el = document.createElement('game-playbook-pathway');
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      containerRef.current.appendChild(el);
      elementRef.current = el;
    });
    return () => {
      if (elementRef.current && containerRef.current?.contains(elementRef.current)) {
        containerRef.current.removeChild(elementRef.current);
      }
      elementRef.current = null;
    };
  }, []);

  useEffect(() => {
    const el = elementRef.current as (HTMLElement & { setParam?: (n: string, v: number) => void }) | null;
    el?.setParam?.('progress', progress);
    el?.setParam?.('streak', streak);
    el?.setParam?.('momentum', Math.max(0.3, progress));
  }, [progress, streak]);

  return <div ref={containerRef} className="w-full h-10 rounded-lg overflow-hidden opacity-50" aria-hidden="true" />;
}

// 3a. Memoized lesson content — avoids re-parsing markdown on every parent render
const LessonContent = memo(function LessonContent({ content, moduleId, lessonIdx }: {
  content: string;
  moduleId: string;
  lessonIdx: number;
}) {
  return <>{renderMarkdown(content, { moduleId, lessonIdx })}</>;
});

export function PlaybookView() {
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

  const loadModules = useAppStore((s) => s.loadPlaybookModules);
  const loadContent = useAppStore((s) => s.loadPlaybookContent);
  const loadProgress = useAppStore((s) => s.loadPlaybookProgress);
  const markComplete = useAppStore((s) => s.markLessonComplete);
  const loadPersonalized = useAppStore((s) => s.loadPersonalizedContent);
  const loadStreetsTier = useAppStore((s) => s.loadStreetsTier);

  const [showTemplates, setShowTemplates] = useState(false);

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
            `Your scoring engine just learned about ${lessonTitle}. Future results will reflect this.`,
          );
        }
      }
    },
    [markComplete, addToast],
  );

  // 3c. Track already-requested personalization keys to avoid duplicate IPC calls
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
          <div className="mb-4 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg text-sm text-[#EF4444]">
            {playbookError}
          </div>
        )}

        {showTemplates && (
          <div className="bg-bg-secondary border border-border rounded-xl p-6">
            <TemplateLibrary />
          </div>
        )}

        {!showTemplates && !activeModuleId && !playbookLoading && (
          <div className="flex flex-col items-center justify-center h-96 text-center">
            <div className="w-16 h-16 bg-[#D4AF37]/10 rounded-2xl flex items-center justify-center mb-4">
              <span className="text-2xl text-[#D4AF37] font-bold">S</span>
            </div>
            <h3 className="text-lg font-semibold text-white mb-2">{t('streets:streets.title')}</h3>
            <p className="text-sm text-text-secondary max-w-md mb-4">
              {t('streets:streets.selectModuleDescription')}{' '}
              {t('streets:streets.selectModule')}
            </p>
            <button
              onClick={() => handleModuleClick('S')}
              className="px-4 py-2 bg-[#D4AF37] text-black text-sm font-medium rounded-lg hover:bg-[#C4A030] transition-colors"
            >
              {t('streets:streets.startWith')}
            </button>
          </div>
        )}

        {!showTemplates && playbookLoading && (
          <div className="flex items-center justify-center h-96">
            <div className="w-6 h-6 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
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
                <span className="px-2 py-1 bg-[#D4AF37]/20 text-[#D4AF37] text-xs font-bold rounded">
                  {playbookContent.module_id}
                </span>
              </div>
              <h2 className="text-xl font-semibold text-white">{playbookContent.title}</h2>
              <p className="text-sm text-text-secondary mt-1">{playbookContent.description}</p>
            </div>

            {/* Lessons */}
            {playbookContent.lessons.map((lesson, idx) => {
              const isCompleted = completedSet.has(idx);
              const pKey = `${playbookContent.module_id}:${idx}`;
              const personalized = personalizedLessons[pKey];
              const lessonContent = personalized?.content ?? lesson.content;

              // Separate temporal blocks by type
              const diffBlocks = personalized?.temporal_blocks.filter(
                (b) => b.block_type.type === 'diff_ribbon',
              ) ?? [];
              const revealBlocks = personalized?.temporal_blocks.filter(
                (b) => b.block_type.type === 'progressive_reveal',
              ) ?? [];
              const echoBlocks = personalized?.temporal_blocks.filter(
                (b) => b.block_type.type === 'feed_echo',
              ) ?? [];

              return (
                <div
                  key={idx}
                  className="bg-bg-secondary border border-border rounded-xl overflow-hidden"
                >
                  {/* Temporal: Diff Ribbon at top */}
                  {diffBlocks.map((b) => (
                    <DiffRibbon key={b.block_id} block={b} />
                  ))}

                  {/* Temporal: Progressive Reveal Banner */}
                  {revealBlocks.map((b) => (
                    <ProgressiveRevealBanner key={b.block_id} block={b} />
                  ))}

                  {/* Lesson header */}
                  <div className="flex items-center gap-3 px-6 py-4 border-b border-border">
                    <button
                      onClick={() => handleLessonToggle(playbookContent.module_id, idx)}
                      aria-label={isCompleted ? `Mark "${lesson.title}" incomplete` : `Mark "${lesson.title}" complete`}
                      aria-pressed={isCompleted}
                      className={`w-5 h-5 rounded border-2 flex items-center justify-center flex-shrink-0 transition-colors ${
                        isCompleted
                          ? 'bg-[#22C55E] border-[#22C55E]'
                          : 'border-[#666] hover:border-[#D4AF37]'
                      }`}
                    >
                      {isCompleted && (
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                      )}
                    </button>
                    <h3 className={`text-sm font-medium flex-1 ${isCompleted ? 'text-text-secondary' : 'text-white'}`}>
                      {lesson.title}
                    </h3>
                    {personalized && <PersonalizationDepthIndicator depth={personalized.depth} />}
                  </div>

                  {/* 3a. Lesson content — memoized markdown rendering */}
                  <div className="px-6 py-5 prose-4da text-sm leading-relaxed text-text-secondary">
                    <LessonContent
                      content={lessonContent}
                      moduleId={playbookContent.module_id}
                      lessonIdx={idx}
                    />
                  </div>

                  {/* L3: Sovereign Insight Cards */}
                  {personalized && personalized.insight_blocks.length > 0 && (
                    <div className="px-6 pb-4">
                      {personalized.insight_blocks.map((block) => (
                        <SovereignInsightCard key={block.block_id} block={block} />
                      ))}
                    </div>
                  )}

                  {/* L4: Sovereign Connection (Mirror) Blocks */}
                  {personalized && personalized.mirror_blocks.length > 0 && (
                    <div className="px-6 pb-4">
                      {personalized.mirror_blocks.map((block) => (
                        <SovereignConnectionBlock key={block.block_id} block={block} />
                      ))}
                    </div>
                  )}

                  {/* L5: Feed Echo Blocks */}
                  {echoBlocks.length > 0 && (
                    <div className="px-6 pb-4">
                      {echoBlocks.map((b) => (
                        <FeedEchoBlock key={b.block_id} block={b} />
                      ))}
                    </div>
                  )}
                </div>
              );
            })}

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
}
