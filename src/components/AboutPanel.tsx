import { useEffect, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { registerGameComponent } from '../lib/game-components';

const GeometryShowcase = lazy(() => import('./geometry/GeometryShowcase').then(m => ({ default: m.GeometryShowcase })));
const ConfigDiagnostics = lazy(() => import('./enterprise/ConfigDiagnostics').then(m => ({ default: m.ConfigDiagnostics })));

export function AboutPanel() {
  const { t } = useTranslation();

  useEffect(() => {
    registerGameComponent('game-simplex-unfold');
    registerGameComponent('game-pentachoron');
  }, []);

  return (
    <div className="space-y-8">
      {/* Logo + Identity — Simplex unfold as living brand mark */}
      <div className="flex flex-col items-center text-center">
        <div
          className="w-28 h-28 mb-4 rounded-2xl overflow-hidden shadow-lg shadow-orange-500/20 border border-border/30"
          role="img"
          aria-label={t('about.logoAlt')}
        >
          <game-simplex-unfold style={{ width: '112px', height: '112px', display: 'block' }} />
        </div>
        <h3 className="text-xl font-semibold text-white">{t('app.title')}</h3>
        <p className="text-sm text-text-secondary mt-1">{t('about.fullName')}</p>
        <p className="text-xs text-text-muted mt-0.5">{t('app.tagline')}</p>
      </div>

      {/* Platonic Architecture — the mathematical identity */}
      <div className="bg-bg-tertiary/30 border border-[#D4AF37]/20 rounded-xl p-4">
        <Suspense fallback={null}>
          <GeometryShowcase />
        </Suspense>
      </div>

      {/* Built With Section */}
      <div className="bg-bg-tertiary/50 border border-border rounded-xl p-5 space-y-4">
        <h4 className="text-sm font-medium text-white tracking-wide uppercase">
          {t('about.attribution')}
        </h4>

        <div className="space-y-3 text-sm text-text-secondary leading-relaxed">
          <p>
            {t('about.attributionCreatorBefore')}{' '}
            <span className="text-white font-medium">{t('about.attributionCreatorName')}</span>{' '}
            {t('about.attributionCreatorAfter')}
          </p>

          <p>
            {t('about.attributionClaudeBefore')}{' '}
            <span className="text-white font-medium">{t('about.attributionClaudeName')}</span>{' '}
            {t('about.attributionClaudeAfter')}
          </p>

          <p className="text-text-secondary text-xs">
            {t('about.attributionLegitimacy')}
          </p>
        </div>

        {/* Attribution Visual */}
        <div className="mt-6 flex items-center justify-center gap-6">
          {/* Human creator */}
          <div className="flex flex-col items-center gap-2">
            <div className="w-14 h-14 rounded-xl bg-bg-secondary border border-border flex items-center justify-center text-2xl">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-white">
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
            </div>
            <span className="text-[10px] text-text-muted font-medium uppercase tracking-wider">{t('about.vision')}</span>
          </div>

          {/* Connection — pentachoron as the 5th vertex linking human + AI */}
          <div className="flex flex-col items-center gap-1">
            <div className="flex items-center gap-1">
              <div className="w-4 h-px bg-gray-600" />
              <div
                className="w-11 h-11 rounded-lg overflow-hidden border border-border/20"
                role="img"
                aria-label={t('about.collaborative')}
              >
                <game-pentachoron style={{ width: '44px', height: '44px', display: 'block' }} />
              </div>
              <div className="w-4 h-px bg-gray-600" />
            </div>
            <span className="text-[9px] text-text-muted">{t('about.collaborative')}</span>
          </div>

          {/* Claude */}
          <div className="flex flex-col items-center gap-2">
            <div className="w-14 h-14 rounded-xl bg-bg-secondary border border-border flex items-center justify-center">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" className="text-[#D97706]">
                <path d="M13.5 2.5L12 4L10.5 2.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                <path d="M12 4v4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                <rect x="6" y="8" width="12" height="10" rx="3" stroke="currentColor" strokeWidth="1.5" />
                <circle cx="9.5" cy="13" r="1" fill="currentColor" />
                <circle cx="14.5" cy="13" r="1" fill="currentColor" />
                <path d="M10 16h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </div>
            <span className="text-[10px] text-text-muted font-medium uppercase tracking-wider">{t('about.engine')}</span>
          </div>
        </div>
      </div>

      {/* Verification Notice */}
      <div className="bg-green-500/5 border border-green-500/20 rounded-lg p-4">
        <div className="flex items-start gap-3">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-green-400 mt-0.5 flex-shrink-0">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
            <polyline points="22 4 12 14.01 9 11.01" />
          </svg>
          <div>
            <p className="text-xs font-medium text-green-400">{t('about.verifiable')}</p>
            <p className="text-xs text-text-muted mt-1">
              {t('about.verifiableDescription')}
            </p>
          </div>
        </div>
      </div>

      {/* Technical Details */}
      <div className="grid grid-cols-2 gap-3 text-xs">
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-text-muted mb-1">{t('about.stack')}</p>
          <p className="text-text-secondary">{t('about.stackValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-text-muted mb-1">{t('about.framework')}</p>
          <p className="text-text-secondary">{t('about.frameworkValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-text-muted mb-1">{t('about.license')}</p>
          <p className="text-text-secondary">{t('about.licenseValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-text-muted mb-1">{t('about.privacyModel')}</p>
          <p className="text-text-secondary">{t('about.privacyValue')}</p>
        </div>
      </div>

      {/* Keyboard Shortcuts */}
      <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-5 space-y-3">
        <h4 className="text-sm font-medium text-white tracking-wide uppercase">
          {t('shortcuts.title')}
        </h4>
        <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
          {[
            ['R', t('shortcuts.runAnalysis')],
            ['F', t('shortcuts.toggleFilter')],
            ['B', t('shortcuts.openBriefing')],
            [',', t('shortcuts.openSettings')],
            ['?', t('shortcuts.showHelp')],
            ['Esc', t('shortcuts.closePanel')],
            ['S', t('shortcuts.saveItem')],
            ['J / K', t('shortcuts.navigateItems')],
          ].map(([key, desc]) => (
            <div key={key} className="flex items-center gap-2">
              <kbd className="px-1.5 py-0.5 bg-bg-secondary rounded border border-border text-text-secondary font-mono text-[11px] min-w-[28px] text-center">
                {key}
              </kbd>
              <span className="text-text-muted">{desc}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Framework + Philosophy */}
      <div className="bg-bg-tertiary/30 border border-orange-500/20 rounded-xl p-4 space-y-3">
        <h4 className="text-sm font-medium text-orange-400 tracking-wide uppercase">
          {t('about.framework', { defaultValue: 'The 4DA Framework' })}
        </h4>
        <p className="text-xs text-text-secondary leading-relaxed">
          {t('about.frameworkDesc', { defaultValue: 'PASIFA scoring, Authority Stack governance, and the Autonomous Operations System — the philosophy behind 4DA, published openly.' })}
        </p>
        <div className="flex gap-2">
          <button
            onClick={() => {
              window.dispatchEvent(new Event('4da:show-framework'));
            }}
            className="flex-1 px-3 py-2 text-xs font-medium text-orange-400 bg-orange-500/10 border border-orange-500/20 rounded-lg hover:bg-orange-500/20 transition-all"
          >
            {t('about.viewFramework', { defaultValue: 'View Framework' })}
          </button>
          <button
            onClick={() => {
              window.dispatchEvent(new Event('4da:show-comparison'));
            }}
            className="flex-1 px-3 py-2 text-xs font-medium text-text-secondary bg-bg-secondary border border-border rounded-lg hover:bg-bg-tertiary transition-all"
          >
            {t('about.viewComparison', { defaultValue: 'Compare' })}
          </button>
        </div>
      </div>

      {/* System Diagnostics — accessible to all users */}
      <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-4">
        <Suspense fallback={null}>
          <ConfigDiagnostics />
        </Suspense>
      </div>

      {/* Version + Copyright */}
      <div className="text-center pt-2 border-t border-border/50">
        <p className="text-xs text-text-muted">
          {t('about.copyright', { version: __APP_VERSION__ })}
        </p>
        <p className="text-[10px] text-text-muted mt-1">
          {t('about.tagline')}
        </p>
        <div className="flex items-center justify-center gap-3 mt-2">
          <a
            href="https://4da.ai/privacy"
            target="_blank"
            rel="noopener noreferrer"
            onClick={(e) => { e.preventDefault(); import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://4da.ai/privacy')); }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.privacyPolicy')}
          </a>
          <span className="text-text-muted text-[10px]">&middot;</span>
          <a
            href="https://4da.ai/terms"
            target="_blank"
            rel="noopener noreferrer"
            onClick={(e) => { e.preventDefault(); import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://4da.ai/terms')); }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.termsOfService')}
          </a>
        </div>
      </div>
    </div>
  );
}
