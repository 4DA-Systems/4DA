import { useEffect, useState } from 'react';
import { useAppStore } from '../store';
import { getGameIcon } from '../lib/game-icons';
import { registerGameComponent } from '../lib/game-components';

export function AchievementsPanel() {
  const gameState = useAppStore(s => s.gameState);
  const loadGameState = useAppStore(s => s.loadGameState);
  const [gpuProgress, setGpuProgress] = useState(false);

  useEffect(() => {
    loadGameState();
  }, [loadGameState]);

  useEffect(() => {
    registerGameComponent('game-achievement-progress').then(() => {
      if (customElements.get('game-achievement-progress')) setGpuProgress(true);
    });
  }, []);

  if (!gameState) {
    return (
      <div className="flex items-center justify-center h-40 text-[#666]">
        Loading achievements...
      </div>
    );
  }

  const { achievements, total_unlocked, total_achievements, current_streak } = gameState;

  return (
    <div className="space-y-4">
      {/* Summary bar */}
      <div className="flex items-center gap-6 px-4 py-3 bg-[#141414] border border-[#2A2A2A] rounded-lg">
        <div>
          <div className="text-2xl font-bold text-white">
            {total_unlocked}<span className="text-sm font-normal text-[#666]">/{total_achievements}</span>
          </div>
          <div className="text-xs text-[#A0A0A0]">Unlocked</div>
        </div>
        <div className="w-px h-8 bg-[#2A2A2A]" />
        <div>
          <div className="text-2xl font-bold text-white">{current_streak}</div>
          <div className="text-xs text-[#A0A0A0]">Day Streak</div>
        </div>
        <div className="flex-1" />
        {gpuProgress ? (
          <game-achievement-progress
            ref={(el: HTMLElement | null) => {
              if (el && 'progress' in el) {
                (el as HTMLElement & { progress: number }).progress =
                  total_achievements > 0 ? total_unlocked / total_achievements : 0;
              }
            }}
            style={{ width: '32px', height: '32px' }}
          />
        ) : (
          <div className="w-32 h-2 bg-[#1F1F1F] rounded-full overflow-hidden">
            <div
              className="h-full bg-[#D4AF37] rounded-full transition-all duration-500"
              style={{ width: `${total_achievements > 0 ? (total_unlocked / total_achievements) * 100 : 0}%` }}
            />
          </div>
        )}
      </div>

      {/* Achievement grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {achievements.map((a) => {
          const icon = getGameIcon(a.icon);
          const pct = a.threshold > 0 ? Math.min(100, (a.progress / a.threshold) * 100) : 0;

          return (
            <div
              key={a.id}
              className={`relative p-4 rounded-lg border transition-all ${
                a.unlocked
                  ? 'bg-[#141414] border-[#D4AF37]/30'
                  : 'bg-[#0F0F0F] border-[#2A2A2A] opacity-60'
              }`}
            >
              <div className="flex items-start gap-3">
                <div className={`text-2xl ${a.unlocked ? '' : 'grayscale'}`}>
                  {icon}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className={`text-sm font-semibold ${a.unlocked ? 'text-white' : 'text-[#666]'}`}>
                      {a.title}
                    </span>
                    {a.unlocked && (
                      <span className="text-xs text-[#D4AF37]">&#x2713;</span>
                    )}
                  </div>
                  <div className="text-xs text-[#A0A0A0] mt-0.5">{a.description}</div>
                  {!a.unlocked && (
                    <div className="mt-2">
                      <div className="flex items-center justify-between text-[10px] text-[#666] mb-1">
                        <span>{a.progress}/{a.threshold}</span>
                        <span>{Math.round(pct)}%</span>
                      </div>
                      <div className="w-full h-1 bg-[#1F1F1F] rounded-full overflow-hidden">
                        <div
                          className="h-full bg-[#D4AF37]/50 rounded-full transition-all duration-300"
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                    </div>
                  )}
                  {a.unlocked && a.unlocked_at && (
                    <div className="text-[10px] text-[#666] mt-1">
                      {new Date(a.unlocked_at).toLocaleDateString()}
                    </div>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
