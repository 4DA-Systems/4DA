// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import type { Settings } from '../../types';
import type { SettingsForm } from './ai-provider-types';
import type { KeyValidation } from './ai-provider-constants';

interface APIKeyInputProps {
  settings: Settings | null;
  settingsForm: SettingsForm;
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsForm>>;
  validation: KeyValidation;
  validateKey: (provider: string, key: string, baseUrl?: string) => void;
}

export function APIKeyInput({
  settings,
  settingsForm,
  setSettingsForm,
  validation,
  validateKey,
}: APIKeyInputProps) {
  const { t } = useTranslation();
  // Truthful storage copy: only claim the OS credential store when the backend
  // verified the key is there; otherwise say it is kept in settings.json.
  const keyInFile = settings?.secret_storage?.secrets?.llm_api_key === 'file_fallback';

  return (
    <div>
      <label htmlFor="ai-api-key" className="text-xs text-text-muted block mb-1.5">{t('settings.ai.apiKey')}</label>
      <input
        id="ai-api-key"
        type="password"
        value={settingsForm.apiKey}
        onChange={(e) => {
          const val = e.target.value;
          setSettingsForm((f) => ({ ...f, apiKey: val }));
          validateKey(
            settingsForm.provider,
            val,
            settingsForm.provider === 'openai-compatible' ? settingsForm.baseUrl : undefined,
          );
        }}
        placeholder={settings?.llm.has_api_key ? t('settings.ai.keySaved') : t('settings.ai.enterKey')}
        className={`w-full px-4 py-2 bg-bg-secondary border rounded-lg text-sm text-text-primary placeholder:text-text-muted focus:border-orange-500 focus:outline-none font-mono ${settings?.llm.has_api_key && !settingsForm.apiKey ? 'border-green-500/40' : 'border-border'}`}
      />
      {/* Saved key indicator — wording follows the backend's storage posture */}
      {/* eslint-disable i18next/no-literal-string */}
      {settings?.llm.has_api_key && !settingsForm.apiKey && validation.status === 'idle' && (
        keyInFile ? (
          <p className="mt-1.5 text-xs text-amber-400">&#x2713; {t('settings.ai.keySavedFile')}</p>
        ) : (
          <p className="mt-1.5 text-xs text-green-400">&#x2713; {t('settings.ai.keySavedSecure')}</p>
        )
      )}
      {/* eslint-enable i18next/no-literal-string */}
      {/* Real-time validation feedback */}
      {validation.status === 'checking' && (
        <div className="flex items-center gap-2 mt-1.5 text-xs text-text-muted">
          <div className="w-3 h-3 border border-orange-500 border-t-transparent rounded-full animate-spin" />
          {validation.message}
        </div>
      )}
      {/* eslint-disable i18next/no-literal-string */}
      {validation.status === 'valid' && (
        <p className="mt-1.5 text-xs text-green-400">&#x2713; {validation.message}</p>
      )}
      {/* eslint-enable i18next/no-literal-string */}
      {validation.status === 'format_error' && (
        <p className="mt-1.5 text-xs text-red-400">{validation.message}</p>
      )}
      {validation.status === 'invalid' && (
        <p className="mt-1.5 text-xs text-amber-400">{validation.message}</p>
      )}
      <p className="mt-2 text-[10px] text-text-muted leading-relaxed">
        {t('settings.ai.privacyNote')}
      </p>
    </div>
  );
}
