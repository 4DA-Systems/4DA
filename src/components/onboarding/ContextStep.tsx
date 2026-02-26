import { useTranslation } from 'react-i18next';

interface ContextStepProps {
  isAnimating: boolean;
  isDiscovering: boolean;
  discoveryResult: string | null;
  onDiscovery: () => void;
  onNext: () => void;
  onBack: () => void;
}

export function ContextStep({
  isAnimating,
  isDiscovering,
  discoveryResult,
  onDiscovery,
  onNext,
  onBack,
}: ContextStepProps) {
  const { t } = useTranslation();

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">{t('onboarding.context.title')}</h2>
      <p className="text-gray-400 mb-6 text-center">
        {t('onboarding.context.subtitle')}
      </p>

      <div className="bg-bg-secondary p-6 rounded-lg mb-4">
        {/* Discovery state visualization */}
        <div className="flex items-center gap-4 mb-6">
          <div className={`w-16 h-16 rounded-full flex items-center justify-center ${
            discoveryResult ? 'bg-green-500/20' : isDiscovering ? 'bg-orange-500/20' : 'bg-bg-tertiary'
          }`}>
            {discoveryResult ? (
              <span className="text-3xl">&#x2713;</span>
            ) : isDiscovering ? (
              <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
            ) : (
              <span className="text-3xl">&#x1f4c1;</span>
            )}
          </div>
          <div className="flex-1">
            <h3 className="text-white font-medium">
              {discoveryResult ? t('onboarding.context.discoveryComplete') : isDiscovering ? t('onboarding.context.scanning') : t('onboarding.context.autoDiscovery')}
            </h3>
            <p className="text-sm text-gray-400 mt-1">
              {discoveryResult || (isDiscovering
                ? t('onboarding.context.lookingFor')
                : t('onboarding.context.scansLocations')
              )}
            </p>
          </div>
        </div>

        {/* Discovery action or result */}
        {!discoveryResult && !isDiscovering && (
          <button
            onClick={onDiscovery}
            className="w-full py-4 bg-orange-500/20 border-2 border-dashed border-orange-500/50 text-orange-300 rounded-lg hover:bg-orange-500/30 hover:border-orange-500 transition-all font-medium"
          >
            <span className="text-lg">&#x1f50d;</span> {t('onboarding.context.scanMyComputer')}
          </button>
        )}

        {isDiscovering && (
          <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div className="h-full bg-orange-500 rounded-full animate-pulse" style={{ width: '60%' }} />
          </div>
        )}

        {discoveryResult && (
          <div className="bg-green-900/20 border border-green-500/30 text-green-300 p-4 rounded-lg">
            <div className="flex items-center gap-2">
              <span className="text-green-500">&#x2713;</span>
              {discoveryResult}
            </div>
            <p className="text-xs text-green-400/70 mt-2">
              {t('onboarding.context.continuousLearning')}
            </p>
          </div>
        )}

        <p className="text-xs text-gray-500 mt-4 text-center">
          {discoveryResult
            ? t('onboarding.context.manageInSettings')
            : t('onboarding.context.skipHint')
          }
        </p>
      </div>

      {/* FAQ Section */}
      <div className="bg-bg-secondary rounded-lg p-4 mb-6">
        <details className="group">
          <summary className="flex items-center justify-between cursor-pointer text-sm text-gray-400 hover:text-gray-300 transition-colors">
            <span className="flex items-center gap-2">
              <span className="text-orange-400">?</span>
              {t('onboarding.context.faqTitle')}
            </span>
            <span className="text-xs group-open:rotate-180 transition-transform">&#x25bc;</span>
          </summary>
          <div className="mt-4 space-y-4 text-sm">
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">{t('onboarding.context.faqFilesTitle')}</h4>
              <p className="text-gray-400 text-xs">
                {t('onboarding.context.faqFilesDesc')}
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">{t('onboarding.context.faqDataTitle')}</h4>
              <p className="text-gray-400 text-xs">
                <span className="text-green-400 font-medium">{t('onboarding.context.faqDataNo')}</span> {t('onboarding.context.faqDataDesc')}
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">{t('onboarding.context.faqUsageTitle')}</h4>
              <p className="text-gray-400 text-xs">
                {t('onboarding.context.faqUsageDesc')}
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">{t('onboarding.context.faqControlTitle')}</h4>
              <p className="text-gray-400 text-xs">
                {t('onboarding.context.faqControlDesc')}
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">{t('onboarding.context.faqTimeTitle')}</h4>
              <p className="text-gray-400 text-xs">
                {t('onboarding.context.faqTimeDesc')}
              </p>
            </div>
          </div>
        </details>
      </div>

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; {t('onboarding.nav.back')}
        </button>
        <div className="flex items-center gap-3">
          {!discoveryResult && (
            <button
              onClick={onNext}
              className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
            >
              {t('onboarding.nav.skipForNow')}
            </button>
          )}
          <button
            onClick={onNext}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
          >
            {t('onboarding.nav.continue')}
          </button>
        </div>
      </div>
    </div>
  );
}
