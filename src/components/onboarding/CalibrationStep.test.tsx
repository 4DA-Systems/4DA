// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// CalibrationStep must never strand the user: a Fix action finishes when its
// command returns (not on an event that may never come), empty or failed
// detection says so, and a failed calibration can be retried in place.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

vi.mock('../../lib/tauri-events', () => ({
  safeListen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('../../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ embeddingMode: 'semantic' }),
}));

import { CalibrationStep } from './CalibrationStep';

function calibration(actionType: string) {
  return {
    grade: 'C', grade_score: 55, aggregate_f1: 0.5, aggregate_precision: 0.5,
    aggregate_recall: 0.5, mean_separation_gap: 0.1, corpus_items: 10,
    personas_tested: 1, per_persona: [], worst_persona: '', best_persona: '',
    rig_requirements: { recommended_model: 'nomic-embed-text' },
    recommendations: [{ priority: 'P0', title: 'Fix it', action_type: actionType }],
    infrastructure_score: 10, context_richness_score: 10, signal_coverage_score: 10,
    discrimination_score: 10, active_signal_axes: [], nearest_persona: '',
  };
}

function backend(overrides: Record<string, () => unknown>) {
  cmdMock.mockImplementation((command: string) => {
    const override = overrides[command];
    if (override) return override();
    if (command === 'get_user_context') return Promise.resolve({ tech_stack: [], interests: [] });
    return Promise.resolve();
  });
}

const renderStep = () =>
  render(<CalibrationStep isAnimating={false} onComplete={vi.fn()} onBack={vi.fn()} />);

describe('CalibrationStep', () => {
  beforeEach(() => { cmdMock.mockReset(); });

  it('never flashes the old "run an analysis first" dead end before calibrating', () => {
    backend({ run_calibration: () => new Promise(() => {}) });
    renderStep();
    expect(screen.getByText('calibration.onboarding.analyzing')).toBeInTheDocument();
  });

  it('the model Fix button finishes when the pull returns, with no done event', async () => {
    let runs = 0;
    backend({
      run_calibration: () => { runs += 1; return Promise.resolve(calibration('pull_embedding_model')); },
      pull_ollama_model: () => Promise.resolve({ success: true }),
    });
    renderStep();
    await act(async () => {});

    await act(async () => {
      fireEvent.click(screen.getByText('calibration.action.fix'));
    });

    expect(screen.queryByText('calibration.action.working')).not.toBeInTheDocument();
    expect(screen.getByText('calibration.action.fix')).not.toBeDisabled();
    expect(runs).toBe(2); // re-calibrated after the pull
  });

  it('tells the user when stack detection finds nothing', async () => {
    backend({
      run_calibration: () => Promise.resolve(calibration('auto_detect_stacks')),
      detect_stack_profiles: () => Promise.resolve([]),
    });
    renderStep();
    await act(async () => {});

    await act(async () => {
      fireEvent.click(screen.getByText('calibration.action.fix'));
    });

    expect(screen.getByRole('status')).toHaveTextContent('calibration.onboarding.noStacksDetected');
  });

  it('reports a failed stack detection instead of swallowing it', async () => {
    backend({
      run_calibration: () => Promise.resolve(calibration('auto_detect_stacks')),
      detect_stack_profiles: () => Promise.reject('scan failed'),
    });
    renderStep();
    await act(async () => {});

    await act(async () => {
      fireEvent.click(screen.getByText('calibration.action.fix'));
    });

    expect(screen.getByRole('alert')).toHaveTextContent('calibration.onboarding.detectFailed');
  });

  it('a failed calibration offers Retry, which recovers', async () => {
    let attempts = 0;
    backend({
      run_calibration: () => {
        attempts += 1;
        return attempts === 1 ? Promise.reject('context build timed out') : Promise.resolve(calibration('auto_detect_stacks'));
      },
    });
    renderStep();
    await act(async () => {});

    expect(screen.getByRole('alert')).toHaveTextContent('context build timed out');
    await act(async () => {
      fireEvent.click(screen.getByText('action.retry'));
    });

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText('C')).toBeInTheDocument();
  });

  it('disables Finish while completion is being saved', () => {
    backend({ run_calibration: () => new Promise(() => {}) });
    render(<CalibrationStep isAnimating={false} finishing onComplete={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByText('onboarding.setup.savingSettings')).toBeDisabled();
  });
});
