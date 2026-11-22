// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * The design tokens the boot screen needs, as CSS custom properties.
 *
 * App.css defines the real tokens, and App.css loads with the app's module
 * graph, which is exactly what the boot screen is waiting for. So the boot
 * screen sets the handful it uses on its own root element. The values mirror
 * src/App.css for both themes; boot-theme.test.ts fails if they drift.
 */
import type { CSSProperties } from 'react';

type BootTokens = Record<`--color-${string}`, string>;

export const BOOT_TOKENS: { dark: BootTokens; light: BootTokens } = {
  dark: {
    '--color-bg-primary': '#0A0A0A',
    '--color-bg-tertiary': '#1F1F1F',
    '--color-text-primary': '#FFFFFF',
    '--color-text-secondary': '#A0A0A0',
    '--color-text-muted': '#8A8A8A',
    '--color-accent-gold': '#D4AF37',
    '--color-border': '#2A2A2A',
    '--color-success': '#22C55E',
    '--color-error': '#EF4444',
    '--color-accent-action': '#F97316',
  },
  light: {
    '--color-bg-primary': '#F6F5F2',
    '--color-bg-tertiary': '#EBE9E4',
    '--color-text-primary': '#141414',
    '--color-text-secondary': '#57544D',
    '--color-text-muted': '#7A776F',
    '--color-accent-gold': '#8F7118',
    '--color-border': '#DDDAD2',
    '--color-success': '#15803D',
    '--color-error': '#B91C1C',
    '--color-accent-action': '#EA580C',
  },
};

/** index.html sets `data-theme` on <html> before first paint. */
export function isLightTheme(): boolean {
  return typeof document !== 'undefined'
    && document.documentElement.getAttribute('data-theme') === 'light';
}

export function bootTokenStyle(light: boolean = isLightTheme()): CSSProperties {
  return light ? BOOT_TOKENS.light : BOOT_TOKENS.dark;
}
