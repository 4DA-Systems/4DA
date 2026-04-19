// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

// Only include languages that have actual translations.
// he (Hebrew), fa (Persian), ur (Urdu) can be added when translations are generated.
const RTL_LANGUAGES = new Set(['ar']);

export function isRTL(lang: string): boolean {
  return RTL_LANGUAGES.has(lang);
}

export function useDirection(): 'ltr' | 'rtl' {
  const { i18n } = useTranslation();
  const dir = isRTL(i18n.language) ? 'rtl' : 'ltr';

  useEffect(() => {
    document.documentElement.dir = dir;
    document.documentElement.lang = i18n.language;
  }, [dir, i18n.language]);

  return dir;
}
