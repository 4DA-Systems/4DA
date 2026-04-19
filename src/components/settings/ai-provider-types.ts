// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { Settings } from '../../types';
import type { OllamaStatus } from '../../hooks/use-settings';
import type { SettingsForm, ModelRegistryData } from '../../store/types';

// Re-export SettingsForm from the canonical location for convenience
export type { SettingsForm } from '../../store/types';

export interface AIProviderSectionProps {
  settings: Settings | null;
  settingsForm: SettingsForm;
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsForm>>;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  checkOllamaStatus: (baseUrl?: string) => void;
  modelRegistry?: ModelRegistryData | null;
  onRefreshRegistry?: () => void;
}
