import { lazy } from 'react';
import type { ToolDescriptor } from '../../types/toolkit';

export const TOOLS: ToolDescriptor[] = [
  // Phase 1 — Pure Frontend
  {
    id: 'json-yaml',
    name: 'JSON Viewer',
    description: 'Format, minify, diff, and explore JSON with a collapsible tree',
    icon: 'braces',
    category: 'formatters',
    pro: false,
    component: lazy(() => import('./tools/JsonYamlViewer')),
    keywords: ['json', 'yaml', 'format', 'minify', 'tree', 'diff', 'prettify'],
  },
  {
    id: 'regex',
    name: 'Regex Playground',
    description: 'Test patterns with live highlighting, capture groups, and match indices',
    icon: 'regex',
    category: 'formatters',
    pro: false,
    component: lazy(() => import('./tools/RegexPlayground')),
    keywords: ['regex', 'regexp', 'pattern', 'match', 'test', 'capture'],
  },
  {
    id: 'color-picker',
    name: 'Color Picker',
    description: 'Pick colors, convert formats, and check WCAG contrast',
    icon: 'palette',
    category: 'formatters',
    pro: false,
    component: lazy(() => import('./tools/ColorPicker')),
    keywords: ['color', 'hex', 'rgb', 'hsl', 'contrast', 'wcag', 'palette'],
  },
  {
    id: 'encoder-decoder',
    name: 'Encode / Decode',
    description: 'Base64, URL encode, JWT decode, SHA-256/512 hashing',
    icon: 'lock',
    category: 'encoders',
    pro: false,
    component: lazy(() => import('./tools/EncoderDecoder')),
    keywords: ['base64', 'url', 'encode', 'decode', 'jwt', 'hash', 'sha'],
  },
  // Phase 3
  {
    id: 'diff-viewer',
    name: 'Diff Viewer',
    description: 'Compare two texts with unified or side-by-side diff',
    icon: 'diff',
    category: 'formatters',
    pro: false,
    component: lazy(() => import('./tools/DiffViewer')),
    keywords: ['diff', 'compare', 'text', 'merge'],
  },
  // Phase 5 — Generators
  {
    id: 'cron-builder',
    name: 'Cron Builder',
    description: 'Build cron expressions visually with human-readable output',
    icon: 'clock',
    category: 'generators',
    pro: false,
    component: lazy(() => import('./tools/CronBuilder')),
    keywords: ['cron', 'schedule', 'timer', 'job'],
  },
  {
    id: 'mock-data',
    name: 'Mock Data',
    description: 'Generate names, emails, UUIDs, timestamps, and JSON payloads',
    icon: 'database',
    category: 'generators',
    pro: false,
    component: lazy(() => import('./tools/MockDataGenerator')),
    keywords: ['mock', 'fake', 'data', 'uuid', 'name', 'email', 'generate'],
  },
  // Phase 2 — System Integration
  {
    id: 'port-scanner',
    name: 'Port Scanner',
    description: 'Scan listening ports, identify processes, and kill rogue services',
    icon: 'network',
    category: 'system',
    pro: false,
    component: lazy(() => import('./tools/PortScanner')),
    keywords: ['port', 'network', 'process', 'pid', 'listen', 'kill', 'netstat'],
  },
  {
    id: 'env-snapshot',
    name: 'Env Snapshot',
    description: 'Capture git, runtime versions, and OS info — copy as markdown',
    icon: 'terminal',
    category: 'system',
    pro: false,
    component: lazy(() => import('./tools/EnvironmentSnapshot')),
    keywords: ['env', 'environment', 'git', 'node', 'version', 'snapshot', 'system'],
  },
  {
    id: 'http-probe',
    name: 'HTTP Probe',
    description: 'Send HTTP requests with full headers, body, and response inspection',
    icon: 'globe',
    category: 'system',
    pro: true,
    component: lazy(() => import('./tools/HttpProbe')),
    keywords: ['http', 'api', 'request', 'rest', 'get', 'post', 'curl', 'fetch'],
  },
];

export function getToolById(id: string): ToolDescriptor | undefined {
  return TOOLS.find((t) => t.id === id);
}
