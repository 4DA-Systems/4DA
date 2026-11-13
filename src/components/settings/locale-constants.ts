// SPDX-License-Identifier: FSL-1.1-Apache-2.0

export const COUNTRIES = [
  { code: 'US', name: 'United States', lang: 'en', currency: 'USD' },
  { code: 'GB', name: 'United Kingdom', lang: 'en', currency: 'GBP' },
  { code: 'DE', name: 'Germany', lang: 'de', currency: 'EUR' },
  { code: 'FR', name: 'France', lang: 'fr', currency: 'EUR' },
  { code: 'NL', name: 'Netherlands', lang: 'en', currency: 'EUR' },
  { code: 'CA', name: 'Canada', lang: 'en', currency: 'CAD' },
  { code: 'AU', name: 'Australia', lang: 'en', currency: 'AUD' },
  { code: 'JP', name: 'Japan', lang: 'ja', currency: 'JPY' },
  { code: 'IN', name: 'India', lang: 'hi', currency: 'INR' },
  { code: 'BR', name: 'Brazil', lang: 'pt-BR', currency: 'BRL' },
  { code: 'IT', name: 'Italy', lang: 'it', currency: 'EUR' },
  { code: 'ES', name: 'Spain', lang: 'es', currency: 'EUR' },
  { code: 'SE', name: 'Sweden', lang: 'en', currency: 'SEK' },
  { code: 'NO', name: 'Norway', lang: 'en', currency: 'NOK' },
  { code: 'DK', name: 'Denmark', lang: 'en', currency: 'DKK' },
  { code: 'CH', name: 'Switzerland', lang: 'de', currency: 'CHF' },
  { code: 'KR', name: 'South Korea', lang: 'ko', currency: 'KRW' },
  { code: 'NZ', name: 'New Zealand', lang: 'en', currency: 'NZD' },
  { code: 'AT', name: 'Austria', lang: 'de', currency: 'EUR' },
  { code: 'BE', name: 'Belgium', lang: 'en', currency: 'EUR' },
  { code: 'IE', name: 'Ireland', lang: 'en', currency: 'EUR' },
  { code: 'PT', name: 'Portugal', lang: 'en', currency: 'EUR' },
  { code: 'FI', name: 'Finland', lang: 'en', currency: 'EUR' },
  { code: 'SG', name: 'Singapore', lang: 'en', currency: 'SGD' },
  { code: 'MX', name: 'Mexico', lang: 'es', currency: 'MXN' },
];

export const CURRENCIES = [
  'USD', 'EUR', 'GBP', 'CAD', 'AUD', 'JPY', 'INR', 'BRL',
  'CHF', 'SEK', 'NOK', 'DKK', 'NZD', 'KRW', 'SGD', 'MXN', 'CNY',
];

export const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'de', name: 'Deutsch' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'pt-BR', name: 'Português (BR)' },
  { code: 'ru', name: 'Русский' },
  { code: 'tr', name: 'Türkçe' },
  { code: 'zh', name: '中文' },
];

export const TRANSLATION_PROVIDERS = [
  { value: 'auto', labelKey: 'settings.translation.providerAuto' },
  { value: 'deepl', labelKey: 'settings.translation.providerDeepL' },
  { value: 'google', labelKey: 'settings.translation.providerGoogle' },
  { value: 'azure', labelKey: 'settings.translation.providerAzure' },
  { value: 'ollama', labelKey: 'settings.translation.providerOllama' },
  { value: 'llm', labelKey: 'settings.translation.providerLLM' },
] as const;

export const PROVIDERS_REQUIRING_KEY = new Set(['deepl', 'google', 'azure']);

/**
 * Whether the user's AI model runs on this machine (mirrors the backend's
 * `llm_egress::provider_is_on_machine`). Every translation provider other
 * than DeepL / Google / Azure translates with that model, so when it is a
 * cloud model, the cloud-translation consent applies to it too.
 */
export function aiRunsOnMachine(llm: { provider: string; base_url: string | null } | null | undefined): boolean {
  if (!llm) return false;
  const url = llm.base_url?.trim();
  if (url) return /^(https?:\/\/)?(localhost|127\.\d+\.\d+\.\d+|\[::1\])(:\d+)?(\/|$)/i.test(url);
  return llm.provider === 'ollama' || llm.provider === 'none' || llm.provider === '';
}

export function getLanguageName(code: string): string {
  return LANGUAGES.find((l) => l.code === code)?.name ?? code;
}
