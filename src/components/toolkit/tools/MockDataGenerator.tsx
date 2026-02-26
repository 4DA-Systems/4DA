import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

type DataType = 'uuid' | 'name' | 'email' | 'timestamp' | 'ip' | 'hex-color' | 'json';

const DATA_TYPES: { id: DataType; label: string }[] = [
  { id: 'uuid', label: 'UUID' },
  { id: 'name', label: 'Name' },
  { id: 'email', label: 'Email' },
  { id: 'timestamp', label: 'Timestamp' },
  { id: 'ip', label: 'IP Address' },
  { id: 'hex-color', label: 'Hex Color' },
  { id: 'json', label: 'JSON Object' },
];

const FIRST_NAMES = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve', 'Frank', 'Grace', 'Henry', 'Iris', 'Jack', 'Kate', 'Liam', 'Maya', 'Noah', 'Olivia', 'Paul'];
const LAST_NAMES = ['Smith', 'Johnson', 'Williams', 'Brown', 'Jones', 'Garcia', 'Miller', 'Davis', 'Rodriguez', 'Martinez', 'Anderson', 'Taylor', 'Thomas'];
const DOMAINS = ['example.com', 'test.org', 'demo.net', 'mail.dev', 'acme.io'];

function rand(min: number, max: number) { return Math.floor(Math.random() * (max - min + 1)) + min; }
function pick<T>(arr: T[]): T { return arr[rand(0, arr.length - 1)]; }

function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
  });
}

function generateOne(type: DataType): string {
  switch (type) {
    case 'uuid': return generateUUID();
    case 'name': return `${pick(FIRST_NAMES)} ${pick(LAST_NAMES)}`;
    case 'email': {
      const first = pick(FIRST_NAMES).toLowerCase();
      const last = pick(LAST_NAMES).toLowerCase();
      return `${first}.${last}@${pick(DOMAINS)}`;
    }
    case 'timestamp': {
      const d = new Date(Date.now() - rand(0, 365 * 24 * 60 * 60 * 1000));
      return d.toISOString();
    }
    case 'ip': return `${rand(1, 255)}.${rand(0, 255)}.${rand(0, 255)}.${rand(1, 254)}`;
    case 'hex-color': return '#' + Math.floor(Math.random() * 0xFFFFFF).toString(16).padStart(6, '0');
    case 'json': return JSON.stringify({
      id: generateUUID(),
      name: `${pick(FIRST_NAMES)} ${pick(LAST_NAMES)}`,
      email: `${pick(FIRST_NAMES).toLowerCase()}@${pick(DOMAINS)}`,
      active: Math.random() > 0.3,
      score: rand(1, 100),
      created: new Date(Date.now() - rand(0, 365 * 24 * 60 * 60 * 1000)).toISOString(),
    }, null, 2);
  }
}

export default function MockDataGenerator() {
  const { t } = useTranslation();
  const [dataType, setDataType] = useState<DataType>('uuid');
  const [count, setCount] = useState(5);
  const [output, setOutput] = useState('');
  const [copied, setCopied] = useState(false);

  const generate = useCallback(() => {
    const results = Array.from({ length: count }, () => generateOne(dataType));
    setOutput(results.join('\n'));
  }, [dataType, count]);

  const copy = () => {
    navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex items-end gap-3">
        <div>
          <label className="block text-xs text-gray-500 mb-1">{t('toolkit.mockData.type')}</label>
          <select
            value={dataType}
            onChange={(e) => setDataType(e.target.value as DataType)}
            className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-white focus:outline-none focus:border-white/30"
          >
            {DATA_TYPES.map((t) => <option key={t.id} value={t.id}>{t.label}</option>)}
          </select>
        </div>
        <div>
          <label className="block text-xs text-gray-500 mb-1">{t('toolkit.mockData.count')}</label>
          <input
            type="number"
            min={1}
            max={100}
            value={count}
            onChange={(e) => setCount(Math.max(1, Math.min(100, Number(e.target.value))))}
            className="w-20 px-3 py-2 text-sm font-mono bg-bg-secondary border border-border rounded-lg text-white focus:outline-none focus:border-white/30"
          />
        </div>
        <button
          onClick={generate}
          className="px-4 py-2 text-sm font-medium bg-white text-black rounded-lg hover:bg-gray-200 transition-colors"
        >
          {t('action.generate')}
        </button>
      </div>

      {/* Output */}
      {output && (
        <div className="relative">
          <button
            onClick={copy}
            className="absolute top-2 right-2 px-2 py-1 text-xs bg-bg-tertiary border border-border rounded text-gray-400 hover:text-white hover:border-white/20 transition-all"
          >
            {copied ? t('action.copied') : t('action.copy')}
          </button>
          <pre className="p-4 text-sm font-mono bg-bg-secondary border border-border rounded-xl text-gray-300 overflow-auto max-h-96 whitespace-pre-wrap">
            {output}
          </pre>
        </div>
      )}
    </div>
  );
}
