import { useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { renderMarkdown } from '../utils/playbook-markdown';
import type { InsightContent } from '../types/personalization';
import { SovereignProfile } from './playbook/SovereignProfile';
import { StreetHealthBadge } from './playbook/StreetHealthBadge';
import { SunsDashboard } from './playbook/SunsDashboard';
import { SovereignInsightCard } from './playbook/SovereignInsightCard';
import { SovereignConnectionBlock } from './playbook/SovereignConnectionBlock';
import { DiffRibbon } from './playbook/DiffRibbon';
import { FeedEchoBlock } from './playbook/FeedEchoBlock';
import { ProgressiveRevealBanner } from './playbook/ProgressiveRevealBanner';
import { PersonalizationDepthIndicator } from './playbook/PersonalizationDepthIndicator';

// Module IDs (static, mirrors backend MODULE_DEFS)
const MODULE_IDS = ['S', 'T', 'R', 'E1', 'E2', 'T2', 'S2'] as const;

function CheckIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-[#22C55E]">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

function ProgressRing({ percentage }: { percentage: number }) {
  const r = 14;
  const circ = 2 * Math.PI * r;
  const offset = circ - (percentage / 100) * circ;
  return (
    <svg width="36" height="36" viewBox="0 0 36 36" className="flex-shrink-0">
      <circle cx="18" cy="18" r={r} fill="none" stroke="#2A2A2A" strokeWidth="3" />
      <circle
        cx="18" cy="18" r={r} fill="none"
        stroke={percentage >= 100 ? '#22C55E' : '#D4AF37'}
        strokeWidth="3"
        strokeDasharray={circ}
        strokeDashoffset={offset}
        strokeLinecap="round"
        transform="rotate(-90 18 18)"
        className="transition-all duration-500"
      />
      <text x="18" y="19" textAnchor="middle" dominantBaseline="middle" fill="#A0A0A0" fontSize="8" fontFamily="Inter">
        {Math.round(percentage)}%
      </text>
    </svg>
  );
}

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

  // Load modules and progress on mount
  useEffect(() => {
    loadModules();
    loadProgress();
  }, [loadModules, loadProgress]);

  const handleModuleClick = useCallback(
    (moduleId: string) => {
      loadContent(moduleId);
    },
    [loadContent],
  );

  const handleLessonToggle = useCallback(
    (moduleId: string, lessonIdx: number) => {
      markComplete(moduleId, lessonIdx);
    },
    [markComplete],
  );

  // Load personalized content for each lesson when module content is available
  useEffect(() => {
    if (!playbookContent) return;
    const moduleId = playbookContent.module_id;
    playbookContent.lessons.forEach((_, idx) => {
      const key = `${moduleId}:${idx}`;
      if (loadPersonalized && !personalizedLessons?.has(key)) {
        loadPersonalized(moduleId, idx);
      }
    });
  }, [playbookContent, personalizedLessons, loadPersonalized]);

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
      const current = new Map(useAppStore.getState().personalizedLessons);
      const lesson = current.get(key);
      if (!lesson) return;

      const updatedBlocks = lesson.insight_blocks.map((block) =>
        block.block_id === block_id ? { ...block, content } : block,
      );
      current.set(key, { ...lesson, insight_blocks: updatedBlocks });
      useAppStore.setState({ personalizedLessons: current });
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

  // Overall progress percentage
  const overallPct = playbookProgress?.overall_percentage ?? 0;

  return (
    <div className="flex gap-6 min-h-[600px]">
      {/* Sidebar - Module Navigation */}
      <aside className="w-64 flex-shrink-0 bg-[#141414] border border-[#2A2A2A] rounded-xl p-4 space-y-2 self-start sticky top-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-white tracking-wide uppercase">{t('streets:streets.title')}</h2>
          <ProgressRing percentage={overallPct} />
        </div>

        {MODULE_IDS.map((modId) => {
          const progress = playbookProgress?.modules.find((m) => m.module_id === modId);
          const pct = progress?.percentage ?? 0;
          const isActive = activeModuleId === modId;
          const moduleData = playbookModules.find((m) => m.id === modId);
          const lessonCount = moduleData?.lesson_count ?? 0;

          return (
            <button
              key={modId}
              onClick={() => handleModuleClick(modId)}
              className={`w-full text-left px-3 py-2.5 rounded-lg transition-all flex items-center gap-3 group ${
                isActive
                  ? 'bg-[#D4AF37]/15 border border-[#D4AF37]/30'
                  : 'hover:bg-[#1F1F1F] border border-transparent'
              }`}
            >
              <span
                className={`w-7 h-7 rounded-md flex items-center justify-center text-xs font-bold flex-shrink-0 ${
                  pct >= 100
                    ? 'bg-[#22C55E]/20 text-[#22C55E]'
                    : isActive
                      ? 'bg-[#D4AF37]/20 text-[#D4AF37]'
                      : 'bg-[#1F1F1F] text-[#A0A0A0]'
                }`}
              >
                {modId}
              </span>
              <div className="flex-1 min-w-0">
                <p className={`text-sm truncate ${isActive ? 'text-white font-medium' : 'text-[#A0A0A0]'}`}>
                  {t(`streets:streets.module.${modId}`)}
                </p>
                <p className="text-[10px] text-[#666]">
                  {lessonCount} {lessonCount !== 1 ? t('streets:streets.lessons').toLowerCase() : t('streets:streets.lesson').toLowerCase()}
                  {pct > 0 && pct < 100 && ` - ${Math.round(pct)}%`}
                </p>
              </div>
              {pct >= 100 && <CheckIcon />}
            </button>
          );
        })}

        {/* Coach upgrade nudge */}
        <div className="mt-4 pt-4 border-t border-[#2A2A2A] space-y-3">
          <p className="text-[10px] text-[#666] text-center">
            {t('streets:streets.freeForever')}
          </p>
          {streetsTier === 'playbook' && (
            <div className="bg-[#D4AF37]/5 border border-[#D4AF37]/20 rounded-lg p-3">
              <p className="text-[10px] font-medium text-[#D4AF37] mb-1.5">
                {t('streets:streets.wantCoaching')}
              </p>
              <p className="text-[10px] text-[#666] mb-2 leading-relaxed">
                {t('streets:streets.coachingDescription')}
              </p>
              <a
                href="https://4da.ai/streets"
                target="_blank"
                rel="noopener noreferrer"
                className="block text-center px-3 py-1.5 text-[10px] font-medium text-black bg-[#D4AF37] rounded hover:bg-[#C4A030] transition-colors"
              >
                {t('streets:streets.upgrade')}
              </a>
            </div>
          )}
        </div>
      </aside>

      {/* Content Area */}
      <main className="flex-1 min-w-0">
        <StreetHealthBadge />

        {playbookError && (
          <div className="mb-4 px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg text-sm text-[#EF4444]">
            {playbookError}
          </div>
        )}

        {!activeModuleId && !playbookLoading && (
          <div className="flex flex-col items-center justify-center h-96 text-center">
            <div className="w-16 h-16 bg-[#D4AF37]/10 rounded-2xl flex items-center justify-center mb-4">
              <span className="text-2xl text-[#D4AF37] font-bold">S</span>
            </div>
            <h3 className="text-lg font-semibold text-white mb-2">{t('streets:streets.title')}</h3>
            <p className="text-sm text-[#A0A0A0] max-w-md mb-4">
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

        {playbookLoading && (
          <div className="flex items-center justify-center h-96">
            <div className="w-6 h-6 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
          </div>
        )}

        {playbookContent && !playbookLoading && (
          <div className="space-y-4">
            {/* Module Header */}
            <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-6">
              <div className="flex items-center gap-3 mb-2">
                <span className="px-2 py-1 bg-[#D4AF37]/20 text-[#D4AF37] text-xs font-bold rounded">
                  {playbookContent.module_id}
                </span>
              </div>
              <h2 className="text-xl font-semibold text-white">{playbookContent.title}</h2>
              <p className="text-sm text-[#A0A0A0] mt-1">{playbookContent.description}</p>
            </div>

            {/* Lessons */}
            {playbookContent.lessons.map((lesson, idx) => {
              const isCompleted = completedSet.has(idx);
              const pKey = `${playbookContent.module_id}:${idx}`;
              const personalized = personalizedLessons?.get(pKey);
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
                  className="bg-[#141414] border border-[#2A2A2A] rounded-xl overflow-hidden"
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
                  <div className="flex items-center gap-3 px-6 py-4 border-b border-[#2A2A2A]">
                    <button
                      onClick={() => handleLessonToggle(playbookContent.module_id, idx)}
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
                    <h3 className={`text-sm font-medium flex-1 ${isCompleted ? 'text-[#A0A0A0]' : 'text-white'}`}>
                      {lesson.title}
                    </h3>
                    {personalized && <PersonalizationDepthIndicator depth={personalized.depth} />}
                  </div>

                  {/* Lesson content (L1/L2 personalized markdown) */}
                  <div className="px-6 py-5 prose-4da text-sm leading-relaxed text-[#A0A0A0]">
                    {renderMarkdown(lessonContent, { moduleId: playbookContent.module_id, lessonIdx: idx })}
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

            {/* Suns Dashboard — always visible in playbook */}
            <SunsDashboard />
          </div>
        )}
      </main>
    </div>
  );
}
