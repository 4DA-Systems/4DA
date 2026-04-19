// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { SetupAIProvider } from './setup-ai-provider';
import { SetupProjects } from './setup-projects';
import { SetupStack } from './setup-stack';
import { SetupInterests } from './setup-interests';
import { SetupExperience } from './setup-experience';
import { SetupLocale } from './setup-locale';
import { useQuickSetup } from './use-quick-setup';

interface QuickSetupStepProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

const roles = ['Developer', 'Security', 'DevOps', 'Data', 'Manager'];

export function QuickSetupStep({ isAnimating, onComplete, onBack }: QuickSetupStepProps) {
  const {
    t,
    aiOpen, setAiOpen,
    projectsOpen, setProjectsOpen,
    stacksOpen, setStacksOpen,
    interestsOpen, setInterestsOpen,
    localeOpen, setLocaleOpen,
    localeConfigured, setLocaleConfigured,
    experienceOpen, setExperienceOpen,
    experienceLevel, setExperienceLevel,
    selectedStacks, setSelectedStacks,
    ollamaStatus,
    provider,
    apiKey,
    pullingModels,
    pullProgress,
    aiConfigured,
    detectedTech,
    discoveryDone,
    suggestions,
    interests,
    newInterest,
    role, setRole,
    error, setError,
    isSaving,
    apiKeyHint,
    skippedDownload,
    removeTag,
    addInterest,
    toggleInterest,
    handleProviderChange,
    handleApiKeyChange,
    handleContinue,
    handleSkipDownload,
    setNewInterest,
  } = useQuickSetup({ isAnimating, onComplete, onBack });

  // --- Section header component ---
  const SectionHeader = ({
    title,
    subtitle,
    isOpen,
    onToggle,
    done,
    warning,
  }: {
    title: string;
    subtitle: string;
    isOpen: boolean;
    onToggle: () => void;
    done: boolean;
    warning?: boolean;
  }) => (
    <button
      onClick={onToggle}
      aria-expanded={isOpen}
      aria-label={title}
      className="w-full flex items-center justify-between p-4 bg-bg-secondary rounded-lg border border-border hover:border-[#3A3A3A] transition-colors"
    >
      <div className="flex items-center gap-3">
        {done ? (
          <span className="w-6 h-6 bg-green-500/20 rounded-full flex items-center justify-center text-green-400 text-xs">
            &#x2713;
          </span>
        ) : warning ? (
          <span className="w-6 h-6 bg-amber-500/20 rounded-full flex items-center justify-center text-amber-400 text-xs">
            &#x25CB;
          </span>
        ) : (
          <span className="w-6 h-6 bg-bg-tertiary rounded-full flex items-center justify-center text-text-muted text-xs">
            &#x25CB;
          </span>
        )}
        <div className="text-start">
          <div className="text-white font-medium text-sm">{title}</div>
          <div className="text-text-muted text-xs">{subtitle}</div>
        </div>
      </div>
      <span className={`text-text-muted text-xs transition-transform ${isOpen ? 'rotate-180' : ''}`}>
        &#x25BC;
      </span>
    </button>
  );

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">{t('onboarding.setup.title')}</h2>
      <p className="text-text-secondary mb-6 text-center">
        {t('onboarding.setup.subtitle')}
      </p>

      {error && (
        <div role="alert" className="mb-4 p-3 bg-red-900/30 border border-red-500/30 rounded-lg text-sm text-red-200 flex items-start gap-2">
          <span className="text-red-400 flex-shrink-0" aria-hidden="true">&#x26a0;</span>
          <span className="flex-1">{error}</span>
          <button onClick={() => setError(null)} aria-label={t('action.dismiss')} className="text-red-400 hover:text-red-300">&times;</button>
        </div>
      )}

      <div className="space-y-3 mb-6 max-h-[55vh] overflow-y-auto pe-1">
        {/* Section 1: AI Provider */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.aiProvider')}
            subtitle={aiConfigured
              ? (provider === 'ollama' ? t('onboarding.setup.localAiReady') : `${provider === 'anthropic' ? 'Anthropic' : 'OpenAI'} ${t('onboarding.setup.configured')}`)
              : ollamaStatus !== null
                ? t('onboarding.setup.basicModeAvailable')
                : t('onboarding.setup.autoDetecting')}
            isOpen={aiOpen}
            onToggle={() => setAiOpen(!aiOpen)}
            done={aiConfigured}
            warning={!aiConfigured && ollamaStatus !== null}
          />
          {aiOpen && (
            <>
              <SetupAIProvider
                ollamaStatus={ollamaStatus}
                provider={provider}
                apiKey={apiKey}
                pullingModels={pullingModels}
                pullProgress={pullProgress}
                onProviderChange={handleProviderChange}
                onApiKeyChange={handleApiKeyChange}
              />
              {apiKeyHint && (
                <p className="mt-1 px-4 text-xs text-amber-400">{apiKeyHint}</p>
              )}
            </>
          )}
        </div>

        {/* Section 2: Your Projects */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourProjects')}
            subtitle={discoveryDone
              ? (detectedTech.length > 0 ? t('onboarding.setup.techDetected', { count: detectedTech.length }) : t('onboarding.setup.discoveryComplete'))
              : t('onboarding.setup.scanning')}
            isOpen={projectsOpen}
            onToggle={() => setProjectsOpen(!projectsOpen)}
            done={discoveryDone}
          />
          {projectsOpen && (
            <SetupProjects
              discoveryDone={discoveryDone}
              detectedTech={detectedTech}
              onRemoveTag={removeTag}
            />
          )}
        </div>

        {/* Section 3: Your Stack */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourStack')}
            subtitle={selectedStacks.length > 0 ? t('onboarding.setup.profilesSelected', { count: selectedStacks.length }) : t('onboarding.setup.autoDetecting')}
            isOpen={stacksOpen}
            onToggle={() => setStacksOpen(!stacksOpen)}
            done={selectedStacks.length > 0}
          />
          <div style={{ display: stacksOpen ? undefined : 'none' }}>
            <SetupStack
              selectedStacks={selectedStacks}
              onSelectionChange={setSelectedStacks}
            />
          </div>
        </div>

        {/* Section: Your Region */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourRegion')}
            subtitle={localeConfigured ? t('onboarding.setup.configured') : t('onboarding.setup.autoDetected')}
            isOpen={localeOpen}
            onToggle={() => setLocaleOpen(!localeOpen)}
            done={localeConfigured}
          />
          <div style={{ display: localeOpen ? undefined : 'none' }}>
            <SetupLocale onLocaleChange={(_c, _l, _cur) => setLocaleConfigured(true)} />
          </div>
        </div>

        {/* Section 4: Your Interests */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourInterests')}
            subtitle={interests.length > 0
              ? t('onboarding.setup.topicsSelected', { count: interests.length })
              : detectedTech.length > 0
                ? t('onboarding.setup.usingTechStack')
                : t('onboarding.setup.suggestedForYou')}
            isOpen={interestsOpen}
            onToggle={() => setInterestsOpen(!interestsOpen)}
            done={interests.length > 0}
            warning={interests.length === 0 && detectedTech.length > 0}
          />
          <div style={{ display: interestsOpen ? undefined : 'none' }}>
            <SetupInterests
              roles={roles}
              role={role}
              interests={interests}
              newInterest={newInterest}
              suggestions={suggestions}
              onRoleChange={setRole}
              onNewInterestChange={setNewInterest}
              onAddInterest={addInterest}
              onToggleInterest={toggleInterest}
            />
          </div>
        </div>

        {/* Section 5: Experience Level */}
        <div>
          <SectionHeader
            title={t('onboarding.setup.yourExperience')}
            subtitle={experienceLevel
              ? t(`onboarding.experience.${experienceLevel}.label`)
              : t('onboarding.setup.helpsScoring')}
            isOpen={experienceOpen}
            onToggle={() => setExperienceOpen(!experienceOpen)}
            done={experienceLevel !== null}
          />
          <div style={{ display: experienceOpen ? undefined : 'none' }}>
            <SetupExperience
              selected={experienceLevel}
              onSelect={setExperienceLevel}
            />
          </div>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          aria-label={t('onboarding.setup.goBack')}
          className="px-6 py-2 text-text-secondary hover:text-white transition-colors"
        >
          &larr; {t('onboarding.nav.back')}
        </button>
        <div className="flex flex-col items-end gap-1.5">
          <button
            onClick={handleContinue}
            disabled={isSaving}
            aria-label={t('onboarding.setup.completeSetup')}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? t('onboarding.setup.savingSettings') : t('onboarding.setup.enter4DA')}
          </button>
          {pullingModels && (
            <button
              onClick={handleSkipDownload}
              aria-label={t('onboarding.setup.skipModelDownload')}
              className="text-xs text-text-muted hover:text-text-secondary transition-colors"
            >
              {t('onboarding.setup.skipModelDownloadLabel')}
            </button>
          )}
          {!pullingModels && !skippedDownload && (
            <p className="text-[11px] text-text-muted">
              {t('onboarding.setup.allSectionsOptional')}
            </p>
          )}
          {skippedDownload && (
            <p className="text-[11px] text-amber-400">
              {t('action.keywordOnly')}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
