/**
 * Tests for currency utility functions.
 *
 * Covers formatCurrency, convertUsdTo, getCurrencySymbol with various currencies.
 */
import { describe, it, expect } from 'vitest';
import { formatCurrency, convertUsdTo, getCurrencySymbol } from '../currency';

describe('formatCurrency', () => {
  it('formats USD correctly', () => {
    expect(formatCurrency(10, 'USD')).toBe('$10.00');
  });

  it('formats EUR with euro sign', () => {
    const result = formatCurrency(10, 'EUR');
    expect(result).toMatch(/^€/);
    expect(result).toMatch(/\d+\.\d{2}$/);
  });

  it('formats GBP with pound sign', () => {
    const result = formatCurrency(10, 'GBP');
    expect(result).toMatch(/^£/);
  });

  it('formats JPY without decimals', () => {
    const result = formatCurrency(10, 'JPY');
    expect(result).toMatch(/^¥\d+$/);
    expect(result).not.toContain('.');
  });

  it('formats KRW without decimals', () => {
    const result = formatCurrency(10, 'KRW');
    expect(result).toMatch(/^₩\d+$/);
    expect(result).not.toContain('.');
  });

  it('formats AUD with A$ prefix', () => {
    expect(formatCurrency(10, 'AUD')).toMatch(/^A\$/);
  });

  it('formats BRL with R$ prefix', () => {
    expect(formatCurrency(10, 'BRL')).toMatch(/^R\$/);
  });

  it('uses $ for unknown currencies', () => {
    expect(formatCurrency(10, 'ZZZ')).toBe('$10.00');
  });

  it('handles zero amount', () => {
    expect(formatCurrency(0, 'USD')).toBe('$0.00');
  });
});

describe('convertUsdTo', () => {
  it('returns same amount for USD', () => {
    expect(convertUsdTo(100, 'USD')).toBe(100);
  });

  it('converts to EUR (rate ~0.92)', () => {
    const result = convertUsdTo(100, 'EUR');
    expect(result).toBeCloseTo(92, 0);
  });

  it('converts to JPY (rate ~149.5)', () => {
    const result = convertUsdTo(100, 'JPY');
    expect(result).toBeCloseTo(14950, 0);
  });

  it('falls back to rate 1.0 for unknown currencies', () => {
    expect(convertUsdTo(50, 'XYZ')).toBe(50);
  });
});

describe('getCurrencySymbol', () => {
  it('returns $ for USD', () => {
    expect(getCurrencySymbol('USD')).toBe('$');
  });

  it('returns correct symbols for major currencies', () => {
    expect(getCurrencySymbol('EUR')).toBe('€');
    expect(getCurrencySymbol('GBP')).toBe('£');
    expect(getCurrencySymbol('JPY')).toBe('¥');
    expect(getCurrencySymbol('INR')).toBe('₹');
  });

  it('returns $ for unknown currency codes', () => {
    expect(getCurrencySymbol('ZZZ')).toBe('$');
  });
});
