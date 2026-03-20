import { memo, useState, useEffect } from 'react';
import { cmd } from '../lib/commands';

type ShieldLevel = 'secure' | 'advisory' | 'critical';

const SHIELD_COLORS: Record<ShieldLevel, string> = {
  secure: '#22C55E',
  advisory: '#D4AF37',
  critical: '#EF4444',
};

export const DepHealthShield = memo(function DepHealthShield() {
  const [level, setLevel] = useState<ShieldLevel>('secure');
  const [count, setCount] = useState(0);

  useEffect(() => {
    cmd('get_dependency_alerts')
      .then((data) => {
        setCount(data.total);
        if (data.total === 0) setLevel('secure');
        else if (data.alerts.some((a) => a.severity === 'critical')) setLevel('critical');
        else setLevel('advisory');
      })
      .catch(() => {});
  }, []);

  const color = SHIELD_COLORS[level];
  const tooltip =
    count === 0
      ? 'All dependencies secure'
      : `${count} advisor${count === 1 ? 'y' : 'ies'} \u2014 click to view`;

  return (
    <button
      type="button"
      onClick={() => document.querySelector('[data-tab="insights"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }))}
      title={tooltip}
      aria-label={tooltip}
      className="inline-flex items-center justify-center w-8 h-8 rounded-lg
        bg-[#141414] border border-[#2A2A2A] hover:border-[#3A3A3A] transition-colors"
    >
      <svg width="14" height="16" viewBox="0 0 14 16" fill="none">
        <path
          d="M7 1L1 3.5V7.5C1 11.1 3.6 14.4 7 15C10.4 14.4 13 11.1 13 7.5V3.5L7 1Z"
          stroke={color}
          strokeWidth="1.5"
          fill={count > 0 ? `${color}20` : 'none'}
          strokeLinejoin="round"
        />
        {count === 0 && (
          <path d="M4.5 8L6.5 10L9.5 6" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        )}
        {count > 0 && (
          <text x="7" y="10.5" textAnchor="middle" fill={color} fontSize="8" fontWeight="bold">
            {count > 9 ? '!' : count}
          </text>
        )}
      </svg>
    </button>
  );
});
