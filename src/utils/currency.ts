/**
 * Frontend currency utilities for STREETS Content Localization.
 *
 * Mirrors the static exchange rates from the Rust backend so the frontend
 * can do quick conversions without a round-trip to Tauri.
 * Rates are USD-based, updated quarterly — no live API calls.
 */

const EXCHANGE_RATES: Record<string, number> = {
  USD: 1.0, EUR: 0.92, GBP: 0.79, CAD: 1.36, AUD: 1.53,
  JPY: 149.5, INR: 83.0, BRL: 4.97, CHF: 0.88, SEK: 10.4,
  NOK: 10.5, DKK: 6.87, NZD: 1.63, KRW: 1320.0, SGD: 1.34,
  MXN: 17.2, CNY: 7.24,
};

const SYMBOLS: Record<string, string> = {
  USD: '$', EUR: '€', GBP: '£', JPY: '¥', INR: '₹',
  BRL: 'R$', KRW: '₩', CHF: 'CHF ', CNY: '¥',
  CAD: 'C$', AUD: 'A$', NZD: 'NZ$', SEK: 'kr ',
  NOK: 'kr ', DKK: 'kr ', SGD: 'S$', MXN: 'MX$',
};

/** Format a USD amount in the target currency with symbol. */
export function formatCurrency(amount: number, currency: string): string {
  const symbol = SYMBOLS[currency] || '$';
  const rate = EXCHANGE_RATES[currency] || 1.0;
  const converted = amount * rate;
  if (['JPY', 'KRW'].includes(currency)) {
    return `${symbol}${Math.round(converted)}`;
  }
  return `${symbol}${converted.toFixed(2)}`;
}

/** Convert a USD amount to a target currency (numeric only). */
export function convertUsdTo(amount: number, targetCurrency: string): number {
  const rate = EXCHANGE_RATES[targetCurrency] || 1.0;
  return amount * rate;
}

/** Get the display symbol for a currency code. */
export function getCurrencySymbol(currency: string): string {
  return SYMBOLS[currency] || '$';
}
