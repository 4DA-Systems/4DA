// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { DiscoveryFolders } from './use-discovery-folders';

interface DiscoveryFolderPickerProps {
  discovery: DiscoveryFolders;
  disabled?: boolean;
}

/** Tick the folders 4DA may scan, add one by path, or look on other drives. */
export function DiscoveryFolderPicker({ discovery, disabled = false }: DiscoveryFolderPickerProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState('');
  const inputId = useId();
  const { folders, loading, toggleFolder, addFolder, findMoreFolders, searchingDrives, drivesSearched } = discovery;

  const submit = () => {
    addFolder(draft);
    setDraft('');
  };

  return (
    <div className="space-y-2 text-start">
      <p className="text-xs text-text-muted">{t('onboarding.projects.consentIntro')}</p>

      {loading ? (
        <p className="text-xs text-text-secondary" aria-live="polite">{t('onboarding.projects.lookingForFolders')}</p>
      ) : folders.length === 0 ? (
        <p className="text-xs text-text-secondary">{t('onboarding.projects.noCandidates')}</p>
      ) : (
        <ul className="max-h-40 overflow-y-auto space-y-1 pe-1" aria-label={t('onboarding.projects.folderListLabel')}>
          {folders.map(folder => (
            <li key={folder.path}>
              <label className="flex items-center gap-2 text-xs text-text-primary cursor-pointer">
                <input
                  type="checkbox"
                  checked={folder.checked}
                  disabled={disabled}
                  onChange={() => toggleFolder(folder.path)}
                  className="accent-orange-500"
                />
                <span className="font-mono truncate" title={folder.path}>{folder.path}</span>
              </label>
            </li>
          ))}
        </ul>
      )}

      <div className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">{t('onboarding.projects.addFolderLabel')}</label>
        <input
          id={inputId}
          type="text"
          value={draft}
          disabled={disabled}
          onChange={e => setDraft(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); submit(); } }}
          placeholder={t('onboarding.projects.addFolderPlaceholder')}
          className="flex-1 min-w-0 px-3 py-1.5 bg-bg-tertiary border border-border rounded-lg text-xs text-text-primary font-mono placeholder-gray-600 focus:border-orange-500 focus:outline-none"
        />
        <button
          type="button"
          onClick={submit}
          disabled={disabled || draft.trim().length === 0}
          className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-text-primary transition-colors disabled:opacity-50"
        >
          {t('onboarding.projects.addFolder')}
        </button>
      </div>

      {!drivesSearched ? (
        <button
          type="button"
          onClick={() => { void findMoreFolders(); }}
          disabled={disabled || searchingDrives}
          className="text-xs text-orange-400 hover:underline disabled:opacity-50"
        >
          {searchingDrives ? t('onboarding.projects.searchingDrives') : t('onboarding.projects.findMore')}
        </button>
      ) : (
        <p className="text-[11px] text-text-muted">{t('onboarding.projects.drivesSearched')}</p>
      )}
    </div>
  );
}
