// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { lazy } from 'react';
import type { ToolDescriptor } from '../../types/toolkit';

export const TOOLS: ToolDescriptor[] = [
  {
    id: 'source-debugger',
    name: 'Source Debugger',
    description: 'Test any RSS/Atom URL and see what 4DA extracts',
    icon: 'rss',
    category: 'intelligence',
    pro: false,
    component: lazy(() => import('./tools/SourceDebugger')),
    keywords: ['rss', 'atom', 'feed', 'source', 'debug', 'test', 'xml'],
  },
  {
    id: 'scoring-sandbox',
    name: 'Scoring Sandbox',
    description: 'Paste a title and see how your interest profile scores it',
    icon: 'target',
    category: 'intelligence',
    pro: false,
    component: lazy(() => import('./tools/ScoringSandbox')),
    keywords: ['score', 'scoring', 'relevance', 'interest', 'pasifa', 'test'],
  },
];

export function getToolById(id: string): ToolDescriptor | undefined {
  return TOOLS.find((t) => t.id === id);
}
