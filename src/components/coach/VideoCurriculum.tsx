import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import type { VideoLesson } from '../../types/coach';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function StatsBar({
  total,
  unlocked,
  watched,
  completionPct,
}: {
  total: number;
  unlocked: number;
  watched: number;
  completionPct: number;
}) {
  const { t } = useTranslation();
  const pct = Math.min(Math.max(completionPct, 0), 100);

  return (
    <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5 mb-5">
      <div className="flex items-center justify-between mb-3">
        <p className="text-[10px] text-[#666] uppercase tracking-wide font-medium">
          {t('coach.video.curriculumProgress')}
        </p>
        <span className="text-xs text-[#A0A0A0] font-mono">{Math.round(pct)}%</span>
      </div>
      <div className="h-1.5 bg-[#1F1F1F] rounded-full overflow-hidden mb-4">
        <div
          className="h-full bg-[#D4AF37] rounded-full transition-all duration-500"
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="grid grid-cols-3 gap-4">
        <div className="text-center">
          <p className="text-lg font-semibold text-white">{total}</p>
          <p className="text-[10px] text-[#666] uppercase tracking-wide">{t('coach.video.total')}</p>
        </div>
        <div className="text-center">
          <p className="text-lg font-semibold text-[#D4AF37]">{unlocked}</p>
          <p className="text-[10px] text-[#666] uppercase tracking-wide">{t('coach.video.unlocked')}</p>
        </div>
        <div className="text-center">
          <p className="text-lg font-semibold text-[#22C55E]">{watched}</p>
          <p className="text-[10px] text-[#666] uppercase tracking-wide">{t('coach.video.watched')}</p>
        </div>
      </div>
    </div>
  );
}

function LockOverlay({ dripDay }: { dripDay: number }) {
  const { t } = useTranslation();
  return (
    <div className="absolute inset-0 z-10 flex flex-col items-center justify-center bg-[#0A0A0A]/80 backdrop-blur-[2px] rounded-xl">
      <svg
        width="24"
        height="24"
        viewBox="0 0 24 24"
        fill="none"
        stroke="#666"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
        <path d="M7 11V7a5 5 0 0 1 10 0v4" />
      </svg>
      <p className="text-xs text-[#666] mt-2 font-medium">{t('coach.video.unlocksOnDay', { day: dripDay })}</p>
    </div>
  );
}

function WatchedBadge() {
  return (
    <div className="absolute top-3 right-3 z-10 w-6 h-6 rounded-full bg-[#22C55E]/20 flex items-center justify-center">
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="#22C55E"
        strokeWidth="3"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <polyline points="20 6 9 17 4 12" />
      </svg>
    </div>
  );
}

function VideoCard({ lesson }: { lesson: VideoLesson }) {
  const progressPct =
    lesson.duration_seconds > 0
      ? Math.min((lesson.watch_progress_seconds / lesson.duration_seconds) * 100, 100)
      : 0;

  return (
    <div className="relative bg-[#141414] border border-[#2A2A2A] rounded-xl p-4 transition-colors hover:border-[#D4AF37]/30">
      {!lesson.unlocked && <LockOverlay dripDay={lesson.drip_day} />}
      {lesson.watched && <WatchedBadge />}

      {/* Title and metadata */}
      <h4 className="text-sm font-semibold text-white mb-2 pr-8 leading-snug">
        {lesson.title}
      </h4>
      <div className="flex items-center gap-3 mb-3">
        <span className="flex items-center gap-1 text-xs text-[#A0A0A0]">
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <circle cx="12" cy="12" r="10" />
            <polyline points="12 6 12 12 16 14" />
          </svg>
          {formatDuration(lesson.duration_seconds)}
        </span>
        <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-[#1F1F1F] text-[#666] border border-[#2A2A2A]">
          Day {lesson.drip_day}
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-1 bg-[#1F1F1F] rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-300 ${
            lesson.watched ? 'bg-[#22C55E]' : 'bg-[#D4AF37]'
          }`}
          style={{ width: `${progressPct}%` }}
        />
      </div>
      <div className="flex items-center justify-between mt-1">
        <span className="text-[10px] text-[#666]">
          {formatDuration(lesson.watch_progress_seconds)}
        </span>
        <span className="text-[10px] text-[#666]">
          {formatDuration(lesson.duration_seconds)}
        </span>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function VideoCurriculum() {
  const { t } = useTranslation();
  const { videoCurriculum, videoStatus } = useAppStore(
    useShallow(s => ({
      videoCurriculum: s.videoCurriculum,
      videoStatus: s.videoStatus,
    })),
  );

  const loadVideoCurriculum = useAppStore(s => s.loadVideoCurriculum);

  useEffect(() => {
    loadVideoCurriculum();
  }, [loadVideoCurriculum]);

  const completionPct =
    videoStatus && videoStatus.total_videos > 0
      ? (videoStatus.watched_count / videoStatus.total_videos) * 100
      : 0;

  return (
    <div className="space-y-5">
      {/* Header */}
      <div>
        <h3 className="text-sm font-semibold text-white">{t('coach.video.title')}</h3>
        <p className="text-xs text-[#666] mt-0.5">
          {t('coach.video.subtitle')}
        </p>
      </div>

      {/* Stats */}
      {videoStatus && (
        <StatsBar
          total={videoStatus.total_videos}
          unlocked={videoStatus.unlocked_count}
          watched={videoStatus.watched_count}
          completionPct={completionPct}
        />
      )}

      {/* Video Grid */}
      {videoCurriculum.length > 0 ? (
        <div className="grid grid-cols-2 xl:grid-cols-3 gap-4">
          {videoCurriculum.map(lesson => (
            <VideoCard key={lesson.id} lesson={lesson} />
          ))}
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-12 h-12 bg-[#D4AF37]/10 rounded-xl flex items-center justify-center mb-3">
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="#D4AF37"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
          </div>
          <p className="text-sm text-[#A0A0A0] max-w-sm">
            {t('coach.video.emptyDescription')}
          </p>
        </div>
      )}
    </div>
  );
}
