// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useEffect, useRef } from 'react';
import { useAppStore } from '../store';
import { cmd } from '../lib/commands';

const MILESTONES = [
  { id: 'first-analysis', check: (_s: unknown) => false }, // checked async below
  { id: 'feedback-10', check: (_s: unknown) => false }, // checked via feedbackCount
];

export const MilestoneOverlay = memo(function MilestoneOverlay() {
  const [active, setActive] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Check milestones on state changes
  const analysisComplete = useAppStore(s => s.appState.analysisComplete);
  const feedbackCount = useAppStore(s => Object.keys(s.feedbackGiven).length);

  useEffect(() => {
    // Check each milestone
    for (const m of MILESTONES) {
      const key = `4da-milestone-${m.id}`;
      if (localStorage.getItem(key)) continue; // Already triggered

      let triggered = false;
      if (m.id === 'first-analysis' && analysisComplete) {
        // Only trigger if this is truly the first analysis
        void cmd('get_intelligence_growth').then((data: { snapshots?: unknown[] }) => {
          if (data?.snapshots?.length && data.snapshots.length <= 1 && !localStorage.getItem(key)) {
            localStorage.setItem(key, Date.now().toString());
            triggerBurst();
          }
        }).catch(() => {});
        continue;
      }
      if (m.id === 'feedback-10' && feedbackCount >= 10) triggered = true;

      if (triggered) {
        localStorage.setItem(key, Date.now().toString());
        triggerBurst();
        break; // Only one milestone at a time
      }
    }
  }, [analysisComplete, feedbackCount]);

  function triggerBurst() {
    setActive(true);
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setActive(false), 800);
  }

  useEffect(() => {
    return () => { if (timeoutRef.current) clearTimeout(timeoutRef.current); };
  }, []);

  if (!active) return null;

  return (
    <div
      className="fixed inset-0 z-50 pointer-events-none flex items-center justify-center"
      style={{ opacity: active ? 1 : 0, transition: 'opacity 300ms ease-out' }}
    >
      <div className="relative w-64 h-64 flex items-center justify-center" aria-hidden="true">
        {/* Expanding gold rings — subtlety over spectacle */}
        <span className="absolute w-20 h-20 rounded-full border-2 border-accent-gold/70 animate-ping" />
        <span
          className="absolute w-36 h-36 rounded-full border border-accent-gold/40 animate-ping"
          style={{ animationDelay: '140ms' }}
        />
        {/* Center glow */}
        <span
          className="absolute w-3 h-3 rounded-full bg-accent-gold"
          style={{ boxShadow: '0 0 24px 6px rgba(212,175,55,0.6)' }}
        />
      </div>
    </div>
  );
});
