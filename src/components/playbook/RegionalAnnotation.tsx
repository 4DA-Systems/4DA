/**
 * RegionalAnnotation — inline annotation showing region-equivalent prices.
 *
 * Renders a small gold-tinted hint next to USD amounts when the user's
 * locale is set to a non-USD currency. Silently renders nothing for USD users.
 *
 * Usage:
 *   <span>$100/month</span>
 *   <RegionalAnnotation usdAmount={100} unit="/month" />
 */

import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface RegionalAnnotationProps {
  /** The amount in USD to convert. */
  usdAmount: number;
  /** Optional unit suffix, e.g. "/kWh", "/month". */
  unit?: string;
}

export function RegionalAnnotation({ usdAmount, unit = '' }: RegionalAnnotationProps) {
  const { t } = useTranslation();
  const [formatted, setFormatted] = useState<string | null>(null);
  const [currency, setCurrency] = useState<string>('USD');

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const locale = await cmd('get_locale');
        if (cancelled) return;
        if (locale.currency === 'USD') return; // No annotation needed

        setCurrency(locale.currency);
        const result = await cmd('format_currency', { amount: usdAmount });
        if (!cancelled) {
          setFormatted(result);
        }
      } catch {
        // Non-fatal — the annotation simply won't appear
      }
    };

    load();
    return () => { cancelled = true; };
  }, [usdAmount]);

  if (!formatted || currency === 'USD') return null;

  return (
    <span
      className="ml-1 text-[10px] opacity-70"
      style={{ color: '#D4AF37' }}
      title={t('playbook.regional.approximateEquivalent', { currency })}
    >
      {t('playbook.regional.inYourRegion', { amount: formatted, unit })}
    </span>
  );
}
