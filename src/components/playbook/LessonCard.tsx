import { renderMarkdown } from '../../utils/playbook-markdown';
import type { PersonalizedLesson } from '../../types/personalization';
import { SovereignInsightCard } from './SovereignInsightCard';
import { SovereignConnectionBlock } from './SovereignConnectionBlock';
import { DiffRibbon } from './DiffRibbon';
import { FeedEchoBlock } from './FeedEchoBlock';
import { ProgressiveRevealBanner } from './ProgressiveRevealBanner';
import { PersonalizationDepthIndicator } from './PersonalizationDepthIndicator';

interface LessonCardProps {
  lesson: { title: string; content: string };
  idx: number;
  moduleId: string;
  isCompleted: boolean;
  personalized: PersonalizedLesson | undefined;
  onToggle: (moduleId: string, idx: number) => void;
}

export function LessonCard({ lesson, idx, moduleId, isCompleted, personalized, onToggle }: LessonCardProps) {
  const lessonContent = personalized?.content ?? lesson.content;

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
    <div className="bg-bg-secondary border border-border rounded-xl overflow-hidden">
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
          onClick={() => onToggle(moduleId, idx)}
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

      {/* Lesson content (L1/L2 personalized markdown) */}
      <div className="px-6 py-5 prose-4da text-sm leading-relaxed text-text-secondary">
        {renderMarkdown(lessonContent, { moduleId, lessonIdx: idx })}
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
}
