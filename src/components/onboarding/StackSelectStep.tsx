import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { StackProfileSummary, StackDetection } from '../../types/stacks';

// Stack profile display metadata
const STACK_META: Record<string, { icon: string; color: string }> = {
  nextjs_fullstack: { icon: 'N', color: '#FFFFFF' },
  rust_systems: { icon: 'Rs', color: '#DEA584' },
  python_ml: { icon: 'Py', color: '#3776AB' },
  go_backend: { icon: 'Go', color: '#00ADD8' },
  react_native: { icon: 'RN', color: '#61DAFB' },
  laravel: { icon: 'Lv', color: '#FF2D20' },
  django: { icon: 'Dj', color: '#092E20' },
  vue_frontend: { icon: 'Vu', color: '#42B883' },
};

interface StackSelectStepProps {
  selected: string[];
  onSelectionChange: (ids: string[]) => void;
  compact?: boolean;
}

export function StackSelectStep({ selected, onSelectionChange, compact }: StackSelectStepProps) {
  const { t } = useTranslation();
  const [profiles, setProfiles] = useState<StackProfileSummary[]>([]);
  const [detections, setDetections] = useState<StackDetection[]>([]);
  const [loading, setLoading] = useState(true);
  const [autoSelected, setAutoSelected] = useState(false);

  // Load profiles and auto-detect on mount
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [profileList, detected] = await Promise.all([
          cmd('get_stack_profiles'),
          cmd('detect_stack_profiles'),
        ]);
        if (cancelled) return;
        setProfiles(profileList);
        setDetections(detected);

        // Auto-select detected profiles if nothing selected yet
        if (selected.length === 0 && detected.length > 0) {
          const autoIds = detected
            .filter(d => d.confidence >= 0.25)
            .map(d => d.profile_id);
          if (autoIds.length > 0) {
            onSelectionChange(autoIds);
            setAutoSelected(true);
          }
        }
      } catch {
        // Non-fatal — profiles will be empty
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleProfile = useCallback((id: string) => {
    const next = selected.includes(id)
      ? selected.filter(s => s !== id)
      : [...selected, id];
    onSelectionChange(next);
  }, [selected, onSelectionChange]);

  const getDetection = (id: string) => detections.find(d => d.profile_id === id);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-text-secondary py-4">
        <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
        {t('onboarding.stack.loading')}
      </div>
    );
  }

  return (
    <div>
      {!compact && (
        <p className="text-xs text-text-muted mb-3">
          {t('onboarding.stack.description')}
        </p>
      )}

      <div className={`grid ${compact ? 'grid-cols-2' : 'grid-cols-2 sm:grid-cols-4'} gap-2`}>
        {profiles.map((profile) => {
          const isSelected = selected.includes(profile.id);
          const detection = getDetection(profile.id);
          const meta = STACK_META[profile.id] || { icon: '?', color: '#8A8A8A' };

          return (
            <button
              key={profile.id}
              onClick={() => toggleProfile(profile.id)}
              className={`relative p-3 rounded-lg text-start transition-all ${
                isSelected
                  ? 'bg-bg-tertiary border-2 border-white'
                  : 'bg-bg-secondary border-2 border-transparent hover:border-border'
              }`}
            >
              {/* Detection badge */}
              {detection && (
                <span className="absolute top-1.5 end-1.5 px-1.5 py-0.5 text-[10px] font-mono bg-green-500/20 text-green-400 rounded">
                  {Math.round(detection.confidence * 100)}%
                </span>
              )}

              {/* Icon */}
              <div
                className="w-8 h-8 rounded-md flex items-center justify-center text-xs font-bold mb-2"
                style={{
                  backgroundColor: `${meta.color}20`,
                  color: meta.color,
                }}
              >
                {meta.icon}
              </div>

              {/* Name */}
              <div className="text-sm font-medium text-white truncate">
                {profile.name}
              </div>

              {/* Tech tags */}
              <div className="flex flex-wrap gap-1 mt-1.5">
                {profile.core_tech.slice(0, 3).map((tech) => (
                  <span
                    key={tech}
                    className="text-[10px] px-1.5 py-0.5 bg-bg-primary text-text-muted rounded"
                  >
                    {tech}
                  </span>
                ))}
              </div>

              {/* Pain points / shifts count */}
              {!compact && (
                <div className="mt-2 text-[10px] text-text-muted">
                  {t('onboarding.stack.painPoints', { count: profile.pain_point_count })} &middot; {t('onboarding.stack.shifts', { count: profile.ecosystem_shift_count })}
                </div>
              )}
            </button>
          );
        })}
      </div>

      {/* Auto-select banner */}
      {autoSelected && !compact && (
        <div className="mt-3 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg">
          <p className="text-xs text-green-400">
            {t('onboarding.stack.autoSelectedBanner')}
          </p>
        </div>
      )}

      {/* Detection summary */}
      {detections.length > 0 && !compact && (
        <p className="text-xs text-text-muted mt-3">
          {t('onboarding.stack.autoDetected', { count: detections.length })}
          {' '}{t('onboarding.stack.clickToToggle')}
        </p>
      )}

      {selected.length === 0 && !compact && (
        <p className="text-xs text-text-muted mt-2">
          {t('onboarding.stack.noSelection')}
        </p>
      )}
    </div>
  );
}
