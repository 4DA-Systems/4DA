import { lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

// Static imports — custom elements are defined at module load time,
// BEFORE React renders. No async, no race conditions, no effects needed.
import '../lib/game-components/logo-mark.js';

const GeometryShowcase = lazy(() => import('./geometry/GeometryShowcase').then(m => ({ default: m.GeometryShowcase })));
const ConfigDiagnostics = lazy(() => import('./enterprise/ConfigDiagnostics').then(m => ({ default: m.ConfigDiagnostics })));

export function AboutPanel() {
  const { t } = useTranslation();
  const setShowSettings = useAppStore(s => s.setShowSettings);

  return (
    <div className="space-y-6">
      {/* Logo + Identity */}
      <div className="flex flex-col items-center text-center">
        <div
          className="w-28 h-28 mb-3 rounded-2xl overflow-hidden shadow-lg shadow-orange-500/20 border border-border/30"
          role="img"
          aria-label={t('about.logoAlt')}
        >
          <game-logo-mark style={{ width: '112px', height: '112px', display: 'block' }} />
        </div>
        <h3 className="text-xl font-semibold text-white">{t('app.title')}</h3>
        <p className="text-sm text-text-secondary mt-1">{t('about.fullName')}</p>
        <p className="text-xs text-text-muted mt-0.5">{t('app.tagline')}</p>
      </div>

      {/* Technical Details — compact 2x2 grid */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="bg-bg-tertiary/30 rounded-lg p-2.5 border border-border/50">
          <p className="text-text-muted mb-0.5">{t('about.stack')}</p>
          <p className="text-text-secondary">{t('about.stackValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-2.5 border border-border/50">
          <p className="text-text-muted mb-0.5">{t('about.framework')}</p>
          <p className="text-text-secondary">{t('about.frameworkValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-2.5 border border-border/50">
          <p className="text-text-muted mb-0.5">{t('about.license')}</p>
          <p className="text-text-secondary">{t('about.licenseValue')}</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-2.5 border border-border/50">
          <p className="text-text-muted mb-0.5">{t('about.privacyModel')}</p>
          <p className="text-text-secondary">{t('about.privacyValue')}</p>
        </div>
      </div>

      {/* Platonic Architecture */}
      <div className="bg-bg-tertiary/30 border border-accent-gold/20 rounded-xl p-4">
        <Suspense fallback={null}>
          <GeometryShowcase />
        </Suspense>
      </div>

      {/* Framework */}
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
              setShowSettings(false);
              window.dispatchEvent(new Event('4da:show-framework'));
            }}
            className="flex-1 px-3 py-2 text-xs font-medium text-orange-400 bg-orange-500/10 border border-orange-500/20 rounded-lg hover:bg-orange-500/20 transition-all"
          >
            {t('about.viewFramework', { defaultValue: 'View Framework' })}
          </button>
          <button
            onClick={() => {
              setShowSettings(false);
              window.dispatchEvent(new Event('4da:show-comparison'));
            }}
            className="flex-1 px-3 py-2 text-xs font-medium text-text-secondary bg-bg-secondary border border-border rounded-lg hover:bg-bg-tertiary transition-all"
          >
            {t('about.viewComparison', { defaultValue: 'Compare' })}
          </button>
        </div>
      </div>

      {/* Attribution — condensed */}
      <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-4">
        <p className="text-xs text-text-secondary leading-relaxed">
          {t('about.attributionCreatorBefore')}{' '}
          <span className="text-white font-medium">{t('about.attributionCreatorName')}</span>{' '}
          {t('about.attributionCreatorAfter')}{' '}
          {t('about.attributionClaudeBefore')}{' '}
          <span className="text-white font-medium">{t('about.attributionClaudeName')}</span>{' '}
          {t('about.attributionClaudeAfter')}
        </p>
      </div>

      {/* System Diagnostics */}
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
        <div className="flex items-center justify-center gap-3 mt-1.5">
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
