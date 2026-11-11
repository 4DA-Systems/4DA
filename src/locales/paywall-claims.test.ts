// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Paywall copy must match what the backend gates (W0-C, 2026-11-11).
//
// - "AI briefings are a Signal feature" was false: `generate_ai_briefing` runs
//   on the user's own AI provider and is not in SIGNAL_FEATURES
//   (settings/license/gating.rs free-floor exceptions, AD-025).
// - The free-tier Preemption notice named "AI-assessed insights and developing
//   trend chains" but not the Upgrade Plan, which only the Signal feed carries
//   (preemption.rs `append_upgrade_plan_items` runs for TierScope::Full only;
//   `compute_preemption_free_floor_feed` is OSV-verified alerts alone).
// - Paywalls said "Start your free trial" although the trial starts by itself
//   on first launch (settings/manager_init.rs) and cannot be started again.
// These tests pin the corrected copy in every locale.
import i18next from 'i18next';
import { describe, expect, it } from 'vitest';

import ar from './ar/ui.json';
import de from './de/ui.json';
import en from './en/ui.json';
import es from './es/ui.json';
import fr from './fr/ui.json';
import hi from './hi/ui.json';
import it_ from './it/ui.json';
import ja from './ja/ui.json';
import ko from './ko/ui.json';
import ptBR from './pt-BR/ui.json';
import ru from './ru/ui.json';
import tr from './tr/ui.json';
import zh from './zh/ui.json';

import arErrors from './ar/errors.json';
import deErrors from './de/errors.json';
import enErrors from './en/errors.json';
import esErrors from './es/errors.json';
import frErrors from './fr/errors.json';
import hiErrors from './hi/errors.json';
import itErrors from './it/errors.json';
import jaErrors from './ja/errors.json';
import koErrors from './ko/errors.json';
import ptBRErrors from './pt-BR/errors.json';
import ruErrors from './ru/errors.json';
import trErrors from './tr/errors.json';
import zhErrors from './zh/errors.json';

// A new locale directory must be added here too.
const UI_FILES: Record<string, Record<string, unknown>> = {
  ar, de, en, es, fr, hi, it: it_, ja, ko, 'pt-BR': ptBR, ru, tr, zh,
};
const ERROR_FILES: Record<string, Record<string, unknown>> = {
  ar: arErrors, de: deErrors, en: enErrors, es: esErrors, fr: frErrors, hi: hiErrors, it: itErrors,
  ja: jaErrors, ko: koErrors, 'pt-BR': ptBRErrors, ru: ruErrors, tr: trErrors, zh: zhErrors,
};
const locales = Object.keys(UI_FILES);

function ui(locale: string): Record<string, string> {
  return (UI_FILES[locale] ?? {}) as Record<string, string>;
}

function errors(locale: string): Record<string, string> {
  return (ERROR_FILES[locale] ?? {}) as Record<string, string>;
}

/** `haystack` names the feature the way this locale's own UI labels it. */
function names(haystack: string | undefined, label: string | undefined): boolean {
  return Boolean(haystack && label && haystack.toLowerCase().includes(label.toLowerCase()));
}

describe('paywall claims in UI copy', () => {
  it('has the locales it expects', () => {
    expect(locales).toContain('en');
    expect(locales).toHaveLength(13);
  });

  it.each(locales)('%s: no string claims AI briefings are a Signal feature', (locale) => {
    expect(ui(locale)['briefing.signalFeatureNote'], locale).toBeUndefined();
  });

  it.each(locales)('%s: the free-tier Preemption notice names the Upgrade Plan', (locale) => {
    const strings = ui(locale);
    const notice = strings['preemption.freeFloor.subtitle'];
    expect(notice, locale).toBeTruthy();
    expect(names(notice, strings['preemption.upgradePlan.title']), `${locale}: ${notice}`).toBe(true);
    expect(notice, locale).toContain('Signal');
  });

  it.each(locales)('%s: the gate card and the trial-end banner list what Signal really gates', (locale) => {
    const strings = ui(locale);
    for (const key of ['pro.upgradeDescription', 'trialExpiry.body']) {
      const value = strings[key];
      expect(value, `${locale} ${key}`).toBeTruthy();
      for (const label of ['preemption.upgradePlan.title', 'preemption.views.blindspots', 'knowledgeGaps.title']) {
        expect(names(value, strings[label]), `${locale} ${key} should name ${label}: ${value}`).toBe(true);
      }
    }
  });

  it('en: the gate card no longer sells AI briefings', () => {
    expect(ui('en')['pro.upgradeDescription']).toBe(
      'Signal adds the Upgrade Plan, AI-assessed alerts, Blind Spots and Learning Opportunities.',
    );
    expect(ui('en')['pro.upgradeDescription']).not.toMatch(/briefing/i);
  });

  it('en: locked-state copy does not offer a trial the app cannot start', () => {
    const strings = ui('en');
    for (const key of ['preemption.locked.subtitle', 'blindspots.locked.subtitle', 'preemption.freeFloor.subtitle']) {
      expect(strings[key], key).toBeTruthy();
      expect(strings[key], key).not.toMatch(/trial/i);
    }
  });

  it.each(locales)('%s: locked-state copy was rewritten, not left on the retired trial sentence', (locale) => {
    // The first sentence of each subtitle is unchanged; the second used to be
    // "Start your free trial to unlock ...". It now ends on a Signal statement.
    const strings = ui(locale);
    expect(names(strings['blindspots.locked.subtitle'], strings['preemption.views.blindspots']), locale).toBe(true);
    expect(strings['blindspots.locked.subtitle'], locale).toContain('Signal');
    expect(strings['preemption.locked.subtitle'], locale).toContain('Signal');
  });

  it.each(locales)('%s: the trial-start failure line moved with the button', (locale) => {
    expect(ui(locale)['pro.trialStartFailed'], locale).toBeTruthy();
    expect(ui(locale)['briefing.trialStartFailed'], locale).toBeUndefined();
    expect(ui(locale)['pro.trialEnded'], locale).toBeTruthy();
  });

  it.each(locales)('%s: a gate rejection has its own sentence', (locale) => {
    const value = errors(locale)['errorMsg.signalRequired'];
    expect(value, locale).toBeTruthy();
    expect(value, locale).toContain('4DA Signal');
  });
});

describe('licence error key', () => {
  // Mirrors src/i18n/index.ts: default namespace `ui`, no fallback namespace.
  async function appI18n() {
    const instance = i18next.createInstance();
    await instance.init({
      lng: 'en',
      fallbackLng: 'en',
      defaultNS: 'ui',
      ns: ['ui', 'errors'],
      resources: { en: { ui: en, errors: enErrors } },
      interpolation: { escapeValue: false },
    });
    return instance;
  }

  it('resolves only with the errors namespace: the bare key printed itself', async () => {
    const i18n = await appI18n();
    expect(i18n.t('errors:error.license.validationFailed', { detail: 'License key cannot be empty' }))
      .toBe('License validation failed: License key cannot be empty');
    // What Settings > License used to call.
    expect(i18n.t('error.license.validationFailed', { detail: 'x' })).toBe('error.license.validationFailed');
  });

  it.each(locales)('%s: keeps the {{detail}} slot', (locale) => {
    expect(errors(locale)['error.license.validationFailed'], locale).toContain('{{detail}}');
  });
});
