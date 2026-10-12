// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Privacy copy must match what the code sends (NETWORK.md §2a, §2c, §4).
//
// Project scanning used to promise "100% local, nothing ever leaves your
// machine", and the cloud-AI consent text said only "titles and short
// snippets" go to the provider. Both were false: scanning feeds package names
// and versions to OSV.dev and the package registries (sources/osv.rs,
// sources/{npm_registry,crates_io,pypi,go_modules}.rs), and the provider also
// receives dependency names, project labels, git-derived topics, interests and
// saved-item titles (analysis_rerank.rs build_rerank_context_summary,
// briefing_prompt.rs). These tests pin the corrected copy in every locale.
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

// A new locale directory must be added here too.
// Values are mostly flat strings; a few locales also hold nested objects, never read here.
const LOCALE_FILES: Record<string, Record<string, unknown>> = {
  ar, de, en, es, fr, hi, it: it_, ja, ko, 'pt-BR': ptBR, ru, tr, zh,
};
const locales = Object.keys(LOCALE_FILES);

function ui(locale: string): Record<string, string> {
  return (LOCALE_FILES[locale] ?? {}) as Record<string, string>;
}

const SCAN_KEYS = [
  'onboarding.choice.scanProjectsDesc',
  'onboarding.projects.consentIntro',
  'preemption.noDeps.body',
  'blindspots.firstDay.scanBody',
  'signals.stack.coldBody',
];

describe('privacy claims in UI copy', () => {
  it('has the locales it expects', () => {
    expect(locales).toContain('en');
    expect(locales).toHaveLength(13);
  });

  it.each(locales)('%s: scan copy names OSV.dev instead of promising nothing leaves', (locale) => {
    const strings = ui(locale);
    for (const key of SCAN_KEYS) {
      const value = strings[key];
      expect(value, `${locale} ${key}`).toBeTruthy();
      expect(value, `${locale} ${key}`).toContain('OSV.dev');
      expect(value, `${locale} ${key}`).not.toMatch(/100\s?%|%100/);
    }
  });

  it.each(locales)('%s: the cloud-AI disclosure exists and is complete', (locale) => {
    const value = ui(locale)['onboarding.setupAi.cloudDataDisclosure'];
    expect(value, locale).toBeTruthy();
    // The cap and the titles-only switch (its label is not translated in the app yet).
    expect(value).toMatch(/2[,.\s]?000/);
    expect(value).toContain('Send only titles to cloud AI');
    expect(value).toMatch(/git/i);
  });

  it('en: the disclosure lists what the provider receives and what it never does', () => {
    const value = ui('en')['onboarding.setupAi.cloudDataDisclosure'];
    for (const part of [
      'dependency names',
      'installed and fixed versions',
      'project labels instead of file paths',
      'git activity',
      'declared interests',
      'items you saved',
      'source code',
      'commit messages',
    ]) {
      expect(value).toContain(part);
    }
  });

  it.each(locales)('%s: comparison page no longer claims opt-in crash reports or no plaintext keys', (locale) => {
    const strings = ui(locale);
    expect(strings['comp.privacy.desc']).toContain('NETWORK.md');
    expect(strings['comp.privacy.desc']).toContain('OSV.dev');
    expect(strings['comp.privacy.desc']).toContain('4da.ai');
    expect(strings['comp.privacy.keysDetail']).toContain('settings.json');
  });

  it('en: no changed key keeps an absolute "nothing leaves" promise', () => {
    const strings = ui('en');
    for (const key of [...SCAN_KEYS, 'comp.privacy.desc', 'comp.privacy.scanDetail']) {
      expect(strings[key], key).not.toMatch(/nothing (ever )?leaves|never transmitted|no 4DA-operated server/i);
    }
  });
});
