import { memo, useRef, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { VoidEngine } from '../void-engine/VoidEngine';
import { VoidBanner } from '../void-engine/VoidBanner';
import type { VoidBannerHandle } from '../void-engine/VoidBanner';
import { OllamaStatus } from '../OllamaStatus';
import { TeamNotificationBell } from '../team/TeamNotificationBell';
import { LearningBadge } from '../LearningBadge';
import { AccuracySparkline } from '../AccuracySparkline';
import { DepHealthShield } from '../DepHealthShield';
import { useVoidSignals } from '../../hooks/use-void-signals';

interface AppHeaderProps {
  monitoring: { enabled: boolean } | null;
  settingsFormProvider: string;
  isPro: boolean;
  tier: string;
  onOpenSettings: () => void;
  proValueBadge: React.ReactNode;
}

export const AppHeader = memo(function AppHeader({
  monitoring,
  settingsFormProvider,
  isPro,
  tier,
  onOpenSettings,
  proValueBadge,
}: AppHeaderProps) {
  const { t } = useTranslation();
  const signal = useVoidSignals();
  const bannerRef = useRef<VoidBannerHandle>(null);
  const headerRef = useRef<HTMLElement>(null);
  const cursorAnimRef = useRef<number>(0);
  const cursorRef = useRef({ x: 0.5, y: 0.5 });

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLElement>) => {
    cancelAnimationFrame(cursorAnimRef.current);
    if (!headerRef.current) return;
    const rect = headerRef.current.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const y = 1 - (e.clientY - rect.top) / rect.height;
    cursorRef.current = { x, y };
    bannerRef.current?.setCursor(x, y);
  }, []);

  const handleMouseLeave = useCallback(() => {
    const start = { ...cursorRef.current };
    const startTime = performance.now();
    const animate = (now: number) => {
      const progress = Math.min((now - startTime) / 800, 1);
      const eased = 1 - Math.pow(1 - progress, 3);
      const x = start.x + (0.5 - start.x) * eased;
      const y = start.y + (0.5 - start.y) * eased;
      cursorRef.current = { x, y };
      bannerRef.current?.setCursor(x, y);
      if (progress < 1) cursorAnimRef.current = requestAnimationFrame(animate);
    };
    cursorAnimRef.current = requestAnimationFrame(animate);
  }, []);

  useEffect(() => {
    return () => cancelAnimationFrame(cursorAnimRef.current);
  }, []);

  return (
    <header
      ref={headerRef}
      className="relative mb-8 rounded-xl overflow-hidden"
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
    >
      {/* Living banner background */}
      <div className="absolute inset-0 z-0 opacity-80">
        <VoidBanner ref={bannerRef} signal={signal} />
      </div>

      {/* Header content on top */}
      <div className="relative z-10 flex items-center justify-between px-4 py-4">
        <div className="flex items-center gap-4">
          <div className="w-12 h-12 flex items-center justify-center flex-shrink-0">
            <VoidEngine size={48} />
          </div>
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-white">{t('app.title')}</h1>
            <p className="text-gray-500 text-sm">{t('app.tagline')}</p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <AccuracySparkline />
          <DepHealthShield />
          <LearningBadge />
          {monitoring?.enabled && (
            <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border border-green-500/30 rounded-lg">
              <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              <span className="text-xs text-green-400 font-medium">{t('header.live')}</span>
            </div>
          )}
          <OllamaStatus provider={settingsFormProvider} />
          {proValueBadge}
          <span className={`px-2 py-1 text-[11px] font-bold uppercase tracking-wider rounded ${
            isPro
              ? 'bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30'
              : 'bg-bg-tertiary text-gray-400 border border-border'
          }`}>
            {tier}
          </span>
          <TeamNotificationBell />
          <button
            data-settings-trigger
            onClick={onOpenSettings}
            className="px-4 py-2 text-sm bg-bg-secondary/80 text-gray-300 border border-border rounded-lg hover:bg-bg-tertiary hover:border-orange-500/30 transition-all backdrop-blur-sm"
          >
            {t('header.settings')}
          </button>
        </div>
      </div>
    </header>
  );
});
