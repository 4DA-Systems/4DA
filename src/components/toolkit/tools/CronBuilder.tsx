import { useState, useMemo } from 'react';

const MINUTE_OPTIONS = ['*', '0', '15', '30', '45'];
const HOUR_OPTIONS = ['*', ...Array.from({ length: 24 }, (_, i) => String(i))];
const DOM_OPTIONS = ['*', ...Array.from({ length: 31 }, (_, i) => String(i + 1))];
const MONTH_OPTIONS = ['*', 'JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];
const DOW_OPTIONS = ['*', 'SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT'];

const PRESETS = [
  { label: 'Every minute', cron: '* * * * *' },
  { label: 'Every hour', cron: '0 * * * *' },
  { label: 'Every day at midnight', cron: '0 0 * * *' },
  { label: 'Every Monday at 9am', cron: '0 9 * * MON' },
  { label: 'Every weekday at 9am', cron: '0 9 * * MON-FRI' },
  { label: 'Every month on the 1st', cron: '0 0 1 * *' },
];

function describeCron(min: string, hour: string, dom: string, month: string, dow: string): string {
  const parts: string[] = [];
  if (min === '*' && hour === '*') parts.push('Every minute');
  else if (min !== '*' && hour === '*') parts.push(`At minute ${min} of every hour`);
  else if (min === '*' && hour !== '*') parts.push(`Every minute during hour ${hour}`);
  else parts.push(`At ${hour.padStart(2, '0')}:${min.padStart(2, '0')}`);

  if (dom !== '*') parts.push(`on day ${dom} of the month`);
  if (month !== '*') parts.push(`in ${month}`);
  if (dow !== '*') parts.push(`on ${dow}`);
  return parts.join(' ');
}

interface FieldProps {
  label: string;
  value: string;
  options: string[];
  onChange: (v: string) => void;
}

function CronField({ label, value, options, onChange }: FieldProps) {
  return (
    <div>
      <label className="block text-xs text-gray-500 mb-1">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full px-2 py-1.5 text-sm font-mono bg-bg-tertiary border border-border rounded-lg text-white focus:outline-none focus:border-white/30"
      >
        {options.map((o) => <option key={o} value={o}>{o}</option>)}
      </select>
    </div>
  );
}

export default function CronBuilder() {
  const [minute, setMinute] = useState('*');
  const [hour, setHour] = useState('*');
  const [dom, setDom] = useState('*');
  const [month, setMonth] = useState('*');
  const [dow, setDow] = useState('*');
  const [copied, setCopied] = useState(false);

  const expression = `${minute} ${hour} ${dom} ${month} ${dow}`;
  const description = useMemo(() => describeCron(minute, hour, dom, month, dow), [minute, hour, dom, month, dow]);

  const applyPreset = (cron: string) => {
    const [m, h, d, mo, dw] = cron.split(' ');
    setMinute(m); setHour(h); setDom(d); setMonth(mo); setDow(dw);
  };

  const copy = () => {
    navigator.clipboard.writeText(expression);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="space-y-5">
      {/* Output */}
      <div className="flex items-center gap-3 p-4 bg-bg-secondary border border-border rounded-xl">
        <code className="flex-1 text-lg font-mono text-white tracking-wider">{expression}</code>
        <button onClick={copy} className="px-3 py-1.5 text-xs bg-bg-tertiary border border-border rounded-lg text-gray-300 hover:text-white hover:border-white/20 transition-all">
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>

      <p className="text-sm text-gray-400">{description}</p>

      {/* Fields */}
      <div className="grid grid-cols-5 gap-3">
        <CronField label="Minute" value={minute} options={MINUTE_OPTIONS} onChange={setMinute} />
        <CronField label="Hour" value={hour} options={HOUR_OPTIONS} onChange={setHour} />
        <CronField label="Day (month)" value={dom} options={DOM_OPTIONS} onChange={setDom} />
        <CronField label="Month" value={month} options={MONTH_OPTIONS} onChange={setMonth} />
        <CronField label="Day (week)" value={dow} options={DOW_OPTIONS} onChange={setDow} />
      </div>

      {/* Presets */}
      <div>
        <h4 className="text-xs text-gray-500 uppercase tracking-wider mb-2">Presets</h4>
        <div className="flex flex-wrap gap-2">
          {PRESETS.map((p) => (
            <button
              key={p.cron}
              onClick={() => applyPreset(p.cron)}
              className={`px-3 py-1.5 text-xs rounded-lg border transition-all ${
                expression === p.cron
                  ? 'bg-white/10 border-white/20 text-white'
                  : 'bg-bg-secondary border-border text-gray-500 hover:text-gray-300 hover:border-white/10'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
