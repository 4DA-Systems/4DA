// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export type { OllamaStatus } from '../../store/types';

export interface PullProgress {
  model: string;
  status: string;
  percent: number;
  done: boolean;
}

export type Step = 'welcome' | 'taste' | 'choice' | 'setup' | 'calibrate';
