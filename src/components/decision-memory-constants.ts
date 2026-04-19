// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export const DECISION_TYPES = [
  'tech_choice',
  'architecture',
  'workflow',
  'pattern',
  'dependency',
] as const;

// Type labels use i18n: decisions.type.tech_choice, decisions.type.architecture, etc.

export const STATUS_STYLES: Record<string, { text: string; bg: string; border: string }> = {
  active: {
    text: 'text-green-400',
    bg: 'bg-green-500/10',
    border: 'border-green-500/20',
  },
  reconsidering: {
    text: 'text-amber-400',
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/20',
  },
  superseded: {
    text: 'text-text-muted',
    bg: 'bg-gray-500/10',
    border: 'border-gray-500/20',
  },
};

export interface NewDecisionForm {
  decision_type: string;
  subject: string;
  decision: string;
  rationale: string;
  confidence: number;
}

export const EMPTY_FORM: NewDecisionForm = {
  decision_type: 'tech_choice',
  subject: '',
  decision: '',
  rationale: '',
  confidence: 0.8,
};
