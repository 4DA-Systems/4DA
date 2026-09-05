// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { cmd } from '../lib/commands';
import { LogoMarkSVG } from './geometry/LogoMarkSVG';

const GeometryShowcase = lazy(() => import('./geometry/GeometryShowcase').then(m => ({ default: m.GeometryShowcase })));
const PrivacySection = lazy(() => import('./settings/PrivacySection').then(m => ({ default: m.PrivacySection })));

export function AboutPanel() {
  const { t } = useTranslation();
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const [showLicenses, setShowLicenses] = useState(false);

  return (
    <div className="space-y-6">
      {/* Logo + Identity */}
      <div className="flex flex-col items-center text-center">
        <LogoMarkSVG size={128} className="mb-3" />
        <h3 className="text-xl font-semibold text-text-primary">{t('app.title')}</h3>
        <p className="text-sm text-text-secondary mt-1">{t('about.fullName')}</p>
        <p className="text-xs text-text-muted mt-0.5">{t('app.tagline')}</p>
      </div>

      {/* Platonic Architecture */}
      <div className="bg-bg-tertiary/30 border border-accent-gold/20 rounded-xl p-4">
        <Suspense fallback={null}>
          <GeometryShowcase />
        </Suspense>
      </div>

      {/* Privacy */}
      <Suspense fallback={null}>
        <PrivacySection />
      </Suspense>

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
            onClick={(e) => { e.preventDefault(); void import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://4da.ai/privacy')); }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.privacyPolicy')}
          </a>
          <span className="text-text-muted text-[10px]">&middot;</span>
          <a
            href="https://4da.ai/terms"
            target="_blank"
            rel="noopener noreferrer"
            onClick={(e) => { e.preventDefault(); void import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://4da.ai/terms')); }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.termsOfService')}
          </a>
          <span className="text-text-muted text-[10px]">&middot;</span>
          <button
            type="button"
            onClick={() => setShowLicenses(true)}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.thirdPartyLicenses', 'Third-Party Licenses')}
          </button>
          <span className="text-text-muted text-[10px]">&middot;</span>
          <button
            type="button"
            onClick={() => {
              setShowSettings(false);
              window.dispatchEvent(new Event('4da:show-framework'));
            }}
            className="text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
          >
            {t('about.viewFramework')}
          </button>
        </div>
        {/* The one sanctioned quit outside the tray menu: window close only hides
            the tray-resident app (exit guard), so this is the same app.exit(0)
            the tray uses — no forced stop, no ghost tray icon. */}
        <button
          type="button"
          onClick={() => {
            void cmd('quit_app');
          }}
          className="mt-3 text-[10px] text-text-muted hover:text-text-secondary transition-colors underline underline-offset-2"
        >
          {t('about.quitApp')}
        </button>
      </div>

      {showLicenses && (
        <Suspense fallback={null}>
          <ThirdPartyLicensesModal onClose={() => setShowLicenses(false)} />
        </Suspense>
      )}
    </div>
  );
}

const ThirdPartyLicensesModal = lazy(() =>
  import('./ThirdPartyLicensesModal').then(m => ({ default: m.ThirdPartyLicensesModal })),
);
