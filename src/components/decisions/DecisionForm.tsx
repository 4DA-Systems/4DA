// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { DECISION_TYPES } from '../decision-memory-constants';
import type { NewDecisionForm } from '../decision-memory-constants';

export interface DecisionFormProps {
  form: NewDecisionForm;
  onFormChange: (form: NewDecisionForm) => void;
  isSubmitting: boolean;
  onSubmit: () => void;
}

export const DecisionForm = memo(function DecisionForm({
  form,
  onFormChange,
  isSubmitting,
  onSubmit,
}: DecisionFormProps) {
  const { t } = useTranslation();

  return (
    <div
      role="form"
      aria-label={t('decisions.record')}
      aria-busy={isSubmitting}
      className="p-4 border-b border-border space-y-3"
    >
      <div className="flex gap-3">
        <select
          value={form.decision_type}
          onChange={(e) => onFormChange({ ...form, decision_type: e.target.value })}
          aria-label={t('decisions.typeLabel', 'Decision type')}
          className="px-3 py-2 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg focus:outline-none focus:border-text-primary/30"
        >
          {DECISION_TYPES.map((dtype) => (
            <option key={dtype} value={dtype}>
              {t(`decisions.type.${dtype}`)}
            </option>
          ))}
        </select>
        <input
          type="text"
          placeholder={t('decisions.subject')}
          aria-label={t('decisions.subject')}
          value={form.subject}
          onChange={(e) => onFormChange({ ...form, subject: e.target.value })}
          className="flex-1 px-3 py-2 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-text-primary/30"
        />
      </div>
      <textarea
        placeholder={t('decisions.whatDecided')}
        aria-label={t('decisions.whatDecided')}
        value={form.decision}
        onChange={(e) => onFormChange({ ...form, decision: e.target.value })}
        rows={2}
        aria-required="true"
        className="w-full px-3 py-2 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-text-primary/30 resize-none"
      />
      <textarea
        placeholder={t('decisions.rationaleOptional')}
        aria-label={t('decisions.rationaleOptional')}
        value={form.rationale}
        onChange={(e) => onFormChange({ ...form, rationale: e.target.value })}
        rows={2}
        className="w-full px-3 py-2 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg placeholder-gray-600 focus:outline-none focus:border-text-primary/30 resize-none"
      />
      <div className="flex items-center gap-3">
        <label className="text-xs text-text-muted">
          {t('decisions.confidence', { value: Math.round(form.confidence * 100) })}
        </label>
        <input
          type="range"
          min={0}
          max={100}
          value={Math.round(form.confidence * 100)}
          onChange={(e) =>
            onFormChange({ ...form, confidence: parseInt(e.target.value, 10) / 100 })
          }
          aria-label={t('decisions.confidence', { value: Math.round(form.confidence * 100) })}
          className="flex-1 accent-white h-1"
        />
        <button
          onClick={onSubmit}
          disabled={isSubmitting || !form.subject.trim() || !form.decision.trim()}
          className="px-4 py-2 text-xs bg-accent-primary text-bg-primary rounded-lg font-medium hover:bg-accent-primary-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          aria-label={t('decisions.saveDecision', 'Save decision')}
        >
          {t('action.save')}
        </button>
      </div>
    </div>
  );
});
