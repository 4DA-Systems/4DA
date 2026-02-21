import { useEffect, useCallback, useMemo } from 'react';
import { useAppStore } from '../store';
import { useShallow } from 'zustand/react/shallow';
import { renderMarkdown } from '../utils/playbook-markdown';
import { SovereignProfile } from './playbook/SovereignProfile';
import { SunsDashboard } from './playbook/SunsDashboard';

// Module metadata (static, mirrors backend MODULE_DEFS)
const MODULES = [
  { id: 'S', label: 'S', title: 'Sovereign Setup', free: true },
  { id: 'T', label: 'T', title: 'Technical Moats', free: false },
  { id: 'R', label: 'R', title: 'Revenue Engines', free: false },
  { id: 'E1', label: 'E1', title: 'Execution Playbook', free: false },
  { id: 'E2', label: 'E2', title: 'Evolving Edge', free: false },
  { id: 'T2', label: 'T2', title: 'Tactical Automation', free: false },
  { id: 'S2', label: 'S2', title: 'Stacking Streams', free: false },
];

function LockIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-[#666]">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </svg>
  );
}

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
  const {
    playbookModules,
    playbookContent,
    playbookProgress,
    playbookLoading,
    playbookError,
    activeModuleId,
  } = useAppStore(
    useShallow((s) => ({
      playbookModules: s.playbookModules,
      playbookContent: s.playbookContent,
      playbookProgress: s.playbookProgress,
      playbookLoading: s.playbookLoading,
      playbookError: s.playbookError,
      activeModuleId: s.activeModuleId,
    })),
  );

  const loadModules = useAppStore((s) => s.loadPlaybookModules);
  const loadContent = useAppStore((s) => s.loadPlaybookContent);
  const loadProgress = useAppStore((s) => s.loadPlaybookProgress);
  const markComplete = useAppStore((s) => s.markLessonComplete);
  const isPro = useAppStore((s) => s.isPro);

  // Load modules and progress on mount
  useEffect(() => {
    loadModules();
    loadProgress();
  }, [loadModules, loadProgress]);

  const handleModuleClick = useCallback(
    (moduleId: string, isFree: boolean) => {
      if (!isFree && !isPro()) return;
      loadContent(moduleId);
    },
    [loadContent, isPro],
  );

  const handleLessonToggle = useCallback(
    (moduleId: string, lessonIdx: number) => {
      markComplete(moduleId, lessonIdx);
    },
    [markComplete],
  );

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
          <h2 className="text-sm font-semibold text-white tracking-wide uppercase">STREETS</h2>
          <ProgressRing percentage={overallPct} />
        </div>

        {MODULES.map((mod) => {
          const progress = playbookProgress?.modules.find((m) => m.module_id === mod.id);
          const pct = progress?.percentage ?? 0;
          const isActive = activeModuleId === mod.id;
          const isLocked = !mod.free && !isPro();
          const moduleData = playbookModules.find((m) => m.id === mod.id);
          const lessonCount = moduleData?.lesson_count ?? 0;

          return (
            <button
              key={mod.id}
              onClick={() => handleModuleClick(mod.id, mod.free)}
              disabled={isLocked}
              className={`w-full text-left px-3 py-2.5 rounded-lg transition-all flex items-center gap-3 group ${
                isActive
                  ? 'bg-[#D4AF37]/15 border border-[#D4AF37]/30'
                  : isLocked
                    ? 'opacity-50 cursor-not-allowed hover:bg-transparent'
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
                {mod.label}
              </span>
              <div className="flex-1 min-w-0">
                <p className={`text-sm truncate ${isActive ? 'text-white font-medium' : 'text-[#A0A0A0]'}`}>
                  {mod.title}
                </p>
                <p className="text-[10px] text-[#666]">
                  {lessonCount} lesson{lessonCount !== 1 ? 's' : ''}
                  {pct > 0 && pct < 100 && ` - ${Math.round(pct)}%`}
                </p>
              </div>
              {isLocked ? <LockIcon /> : pct >= 100 ? <CheckIcon /> : null}
            </button>
          );
        })}

        {/* Free badge */}
        <div className="mt-4 pt-4 border-t border-[#2A2A2A]">
          <p className="text-[10px] text-[#666] text-center">
            Module S is free. Unlock all with <span className="text-[#D4AF37]">Pro</span>.
          </p>
        </div>
      </aside>

      {/* Content Area */}
      <main className="flex-1 min-w-0">
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
            <h3 className="text-lg font-semibold text-white mb-2">STREETS Playbook</h3>
            <p className="text-sm text-[#A0A0A0] max-w-md mb-4">
              Seven modules to build independent developer income.
              Select a module from the sidebar to begin.
            </p>
            <button
              onClick={() => handleModuleClick('S', true)}
              className="px-4 py-2 bg-[#D4AF37] text-black text-sm font-medium rounded-lg hover:bg-[#C4A030] transition-colors"
            >
              Start with Sovereign Setup
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
                {playbookContent.is_free && (
                  <span className="px-2 py-0.5 bg-[#22C55E]/15 text-[#22C55E] text-[10px] font-medium rounded">
                    FREE
                  </span>
                )}
              </div>
              <h2 className="text-xl font-semibold text-white">{playbookContent.title}</h2>
              <p className="text-sm text-[#A0A0A0] mt-1">{playbookContent.description}</p>
            </div>

            {/* Lessons */}
            {playbookContent.lessons.map((lesson, idx) => {
              const isCompleted = completedSet.has(idx);
              return (
                <div
                  key={idx}
                  className="bg-[#141414] border border-[#2A2A2A] rounded-xl overflow-hidden"
                >
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
                    <h3 className={`text-sm font-medium ${isCompleted ? 'text-[#A0A0A0]' : 'text-white'}`}>
                      {lesson.title}
                    </h3>
                  </div>
                  {/* Lesson content */}
                  <div className="px-6 py-5 prose-4da text-sm leading-relaxed text-[#A0A0A0]">
                    {renderMarkdown(lesson.content, { moduleId: playbookContent.module_id, lessonIdx: idx })}
                  </div>
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
