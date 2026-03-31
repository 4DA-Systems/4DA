/**
 * Locale-aware date formatting utilities.
 * Uses Intl.DateTimeFormat with the user's i18next language for consistent
 * date display across all languages.
 */
import i18n from 'i18next';

/** Get the current user locale as a BCP-47 tag for Intl APIs. */
function getUserLocale(): string {
  const lang = i18n.language;
  // Map our language codes to BCP-47 locale tags
  const localeMap: Record<string, string> = {
    'en': 'en-US',
    'ar': 'ar-SA',
    'de': 'de-DE',
    'es': 'es-ES',
    'fr': 'fr-FR',
    'hi': 'hi-IN',
    'it': 'it-IT',
    'ja': 'ja-JP',
    'ko': 'ko-KR',
    'pt-BR': 'pt-BR',
    'ru': 'ru-RU',
    'tr': 'tr-TR',
    'zh': 'zh-CN',
  };
  return localeMap[lang] || lang || 'en-US';
}

/** Format a date string or Date as a short date (e.g., "Mar 31, 2026" or locale equivalent). */
export function formatLocalDate(dateInput: string | Date): string {
  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(getUserLocale(), {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(date);
}

/** Format a date string or Date as full date+time (e.g., "Mar 31, 2026, 2:30 PM"). */
export function formatLocalDateTime(dateInput: string | Date): string {
  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(getUserLocale(), {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  }).format(date);
}

/** Format a date as month + year (e.g., "March 2026" or locale equivalent). */
export function formatLocalMonthYear(dateInput: string | Date): string {
  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(getUserLocale(), {
    month: 'long',
    year: 'numeric',
  }).format(date);
}

/** Format a date as relative time (e.g., "3 days ago"). Uses Intl.RelativeTimeFormat. */
export function formatRelativeDate(dateInput: string | Date): string {
  const date = typeof dateInput === 'string' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';

  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffSecs = Math.round(diffMs / 1000);
  const diffMins = Math.round(diffSecs / 60);
  const diffHours = Math.round(diffMins / 60);
  const diffDays = Math.round(diffHours / 24);

  const rtf = new Intl.RelativeTimeFormat(getUserLocale(), { numeric: 'auto' });

  if (Math.abs(diffSecs) < 60) return rtf.format(-diffSecs, 'second');
  if (Math.abs(diffMins) < 60) return rtf.format(-diffMins, 'minute');
  if (Math.abs(diffHours) < 24) return rtf.format(-diffHours, 'hour');
  if (Math.abs(diffDays) < 30) return rtf.format(-diffDays, 'day');

  return formatLocalDate(date);
}

/** Format a number with locale-appropriate grouping (e.g., 1,000 vs 1.000). */
export function formatLocalNumber(num: number): string {
  return new Intl.NumberFormat(getUserLocale()).format(num);
}
