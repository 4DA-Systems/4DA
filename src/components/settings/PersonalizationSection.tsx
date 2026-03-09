import { cmd } from '../../lib/commands';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';

export function PersonalizationSection() {
  const { t } = useTranslation();
  const { userContext, suggestedInterests, newInterest, newExclusion, newTechStack, newRole } = useAppStore(
    useShallow(s => ({
      userContext: s.userContext,
      suggestedInterests: s.suggestedInterests,
      newInterest: s.newInterest,
      newExclusion: s.newExclusion,
      newTechStack: s.newTechStack,
      newRole: s.newRole,
    })),
  );
  const setNewInterest = useAppStore(s => s.setNewInterest);
  const setNewExclusion = useAppStore(s => s.setNewExclusion);
  const setNewTechStack = useAppStore(s => s.setNewTechStack);
  const setNewRole = useAppStore(s => s.setNewRole);
  const addInterest = useAppStore(s => s.addInterest);
  const removeInterest = useAppStore(s => s.removeInterest);
  const addExclusion = useAppStore(s => s.addExclusion);
  const removeExclusion = useAppStore(s => s.removeExclusion);
  const addTechStack = useAppStore(s => s.addTechStack);
  const removeTechStack = useAppStore(s => s.removeTechStack);
  const updateRole = useAppStore(s => s.updateRole);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const loadSuggestedInterests = useAppStore(s => s.loadSuggestedInterests);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);

  const handleAddSuggestion = async (topic: string) => {
    try {
      await cmd('add_interest', { topic });
      await loadUserContext();
      await loadSuggestedInterests();
      setSettingsStatus(t('settings.personalization.interestAdded'));
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const handleDismissSuggestion = async (topic: string) => {
    try {
      await cmd('add_exclusion', { topic });
      await loadUserContext();
      await loadSuggestedInterests();
      setSettingsStatus(t('settings.personalization.suggestionDismissed'));
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  };

  const undeclaredSuggestions = suggestedInterests.filter(s => !s.already_declared);
  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-blue-400">&#x1f3af;</span>
        </div>
        <div>
          <h3 className="text-white font-medium">{t('settings.personalization.title')}</h3>
          <p className="text-text-muted text-sm mt-1">
            {t('settings.personalization.description')}
          </p>
        </div>
      </div>

      {userContext ? (
        <div className="space-y-5">
          {/* Role */}
          <div>
            <label className="text-xs text-text-secondary block mb-2">{t('settings.personalization.role')}</label>
            <div className="flex gap-2">
              <input
                type="text"
                aria-label={t('settings.personalization.role')}
                value={newRole}
                onChange={(e) => setNewRole(e.target.value)}
                placeholder={t('settings.personalization.rolePlaceholder')}
                className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-blue-500/50 focus:outline-none transition-colors"
              />
              <button
                onClick={updateRole}
                className="px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-blue-500/30 transition-all"
              >
                {t('settings.personalization.set')}
              </button>
            </div>
          </div>

          {/* Tech Stack */}
          <div>
            <label className="text-xs text-text-secondary block mb-2">{t('settings.personalization.techStack')}</label>
            <div className="flex gap-2 mb-3">
              <input
                type="text"
                aria-label={t('settings.personalization.addTech')}
                value={newTechStack}
                onChange={(e) => setNewTechStack(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addTechStack()}
                placeholder={t('settings.personalization.techPlaceholder')}
                className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-blue-500/50 focus:outline-none transition-colors"
              />
              <button
                onClick={addTechStack}
                className="px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-blue-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {userContext.tech_stack.map((tech) => (
                <span
                  key={tech}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-orange-500/10 text-orange-400 text-xs rounded-md border border-orange-500/20 group"
                >
                  {tech}
                  <button
                    onClick={() => removeTechStack(tech)}
                    aria-label={t('settings.personalization.removeTech', { name: tech })}
                    className="text-orange-400/50 hover:text-red-400 transition-colors"
                  >
                    &times;
                  </button>
                </span>
              ))}
              {userContext.tech_stack.length === 0 && (
                <span className="text-sm text-text-muted">{t('settings.personalization.noTech')}</span>
              )}
            </div>
          </div>

          {/* Interests */}
          <div>
            <div className="flex items-center gap-2 mb-2">
              <label className="text-xs text-text-secondary">{t('settings.personalization.interests')}</label>
              <span className="px-1.5 py-0.5 text-[10px] bg-green-500/20 text-green-400 rounded">{userContext.interests.length}</span>
            </div>
            <div className="flex gap-2 mb-3">
              <input
                type="text"
                aria-label={t('settings.personalization.addInterest')}
                value={newInterest}
                onChange={(e) => setNewInterest(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addInterest()}
                placeholder={t('settings.personalization.interestPlaceholder')}
                className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-green-500/50 focus:outline-none transition-colors"
              />
              <button
                onClick={addInterest}
                className="px-4 py-2.5 text-sm bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto">
              {userContext.interests.map((interest) => (
                <span
                  key={interest.topic}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-green-500/10 text-green-400 text-xs rounded-md border border-green-500/20 group"
                  title={interest.has_embedding ? t('settings.personalization.hasEmbedding') : t('settings.personalization.noEmbedding')}
                >
                  {interest.has_embedding && <span className="w-1.5 h-1.5 bg-green-400 rounded-full" />}
                  {interest.topic}
                  <button
                    onClick={() => removeInterest(interest.topic)}
                    aria-label={t('settings.personalization.removeInterest', { name: interest.topic })}
                    className="text-green-400/50 hover:text-red-400 transition-colors"
                  >
                    &times;
                  </button>
                </span>
              ))}
              {userContext.interests.length === 0 && (
                <span className="text-sm text-text-muted">{t('settings.personalization.noInterests')}</span>
              )}
            </div>
          </div>

          {/* Suggested Interests */}
          {undeclaredSuggestions.length > 0 && (
            <div>
              <h4 className="text-xs text-text-secondary font-medium mb-2">{t('settings.personalization.suggestedInterests')}</h4>
              <p className="text-[10px] text-text-muted mb-2">
                {t('settings.personalization.suggestedDescription')}
              </p>
              <div className="space-y-1">
                {undeclaredSuggestions.slice(0, 5).map((suggestion) => (
                  <div key={suggestion.topic} className="flex items-center justify-between py-1.5 px-2.5 rounded-md bg-bg-tertiary border border-border">
                    <div className="min-w-0 flex-1 mr-2">
                      <span className="text-sm text-white">{suggestion.topic}</span>
                      <span className="text-[10px] text-text-muted ml-2 truncate">{suggestion.source}</span>
                    </div>
                    <div className="flex gap-1 flex-shrink-0">
                      <button
                        onClick={() => handleAddSuggestion(suggestion.topic)}
                        aria-label={t('settings.personalization.addSuggestion', { name: suggestion.topic })}
                        className="text-xs px-2 py-0.5 rounded bg-border text-success hover:bg-[#333] transition-colors"
                      >
                        {t('action.add')}
                      </button>
                      <button
                        onClick={() => handleDismissSuggestion(suggestion.topic)}
                        aria-label={t('settings.personalization.dismissSuggestion', { name: suggestion.topic })}
                        className="text-xs px-2 py-0.5 rounded bg-border text-text-muted hover:bg-[#333] transition-colors"
                      >
                        {t('action.dismiss')}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Exclusions */}
          <div>
            <div className="flex items-center gap-2 mb-2">
              <label className="text-xs text-text-secondary">{t('settings.personalization.exclusions')}</label>
              <span className="px-1.5 py-0.5 text-[10px] bg-red-500/20 text-red-400 rounded">{userContext.exclusions.length}</span>
            </div>
            <div className="flex gap-2 mb-3">
              <input
                type="text"
                aria-label={t('settings.personalization.addExclusion')}
                value={newExclusion}
                onChange={(e) => setNewExclusion(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addExclusion()}
                placeholder={t('settings.personalization.exclusionPlaceholder')}
                className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-red-500/50 focus:outline-none transition-colors"
              />
              <button
                onClick={addExclusion}
                className="px-4 py-2.5 text-sm bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
              >
                {t('settings.personalization.block')}
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {userContext.exclusions.map((exclusion) => (
                <span
                  key={exclusion}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 bg-red-500/10 text-red-400 text-xs rounded-md border border-red-500/20 group"
                >
                  {exclusion}
                  <button
                    onClick={() => removeExclusion(exclusion)}
                    aria-label={t('settings.personalization.removeExclusion', { name: exclusion })}
                    className="text-red-400/50 hover:text-white transition-colors"
                  >
                    &times;
                  </button>
                </span>
              ))}
              {userContext.exclusions.length === 0 && (
                <span className="text-sm text-text-muted">{t('settings.personalization.noExclusions')}</span>
              )}
            </div>
          </div>
        </div>
      ) : (
        <div className="text-sm text-text-muted">{t('settings.personalization.loading')}</div>
      )}
    </div>
  );
}
