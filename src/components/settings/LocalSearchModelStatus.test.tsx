// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// AD-054 decision 7: the local search model is not in the installer. Settings
// says whether it is on disk, downloading (N%), ready or failed, and offers
// the download. It never claims "ready" it cannot see.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, number>) => (p ? `${k} ${JSON.stringify(p)}` : k),
  }),
}));
const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { LocalSearchModelStatus, modelStatusView } from './LocalSearchModelStatus';
import type { EmbeddingEngineStatusDto } from '../../lib/commands';

const MB = 1_048_576;
const status = (state: EmbeddingEngineStatusDto['state'], done = 0, total = 0, message: string | null = null) => ({
  state,
  message,
  bytes_downloaded: done,
  bytes_total: total,
});

describe('modelStatusView', () => {
  it('maps every engine state to an honest label and the right action', () => {
    expect(modelStatusView(status('not_downloaded'))).toMatchObject({ key: 'settings.ai.localModel.notDownloaded', action: 'download' });
    expect(modelStatusView(status('ready'))).toMatchObject({ key: 'settings.ai.localModel.ready', action: null });
    expect(modelStatusView(status('idle'))).toMatchObject({ key: 'settings.ai.localModel.onDisk', action: null });
    expect(modelStatusView(status('failed'))).toMatchObject({ key: 'settings.ai.localModel.failed', action: 'retry' });
    expect(modelStatusView(status('unavailable'))).toMatchObject({ key: 'settings.ai.localModel.unavailable', action: null });
    expect(modelStatusView(null)).toMatchObject({ key: 'settings.ai.localModel.checking', action: null });
  });

  it('reports download progress as a percentage and MB', () => {
    expect(modelStatusView(status('preparing', 131 * MB, 262 * MB))).toEqual({
      key: 'settings.ai.localModel.downloading',
      params: { percent: 50, done: 131, total: 262 },
      tone: 'pending',
      action: null,
    });
    expect(modelStatusView(status('preparing')).key).toBe('settings.ai.localModel.loading');
  });
});

describe('LocalSearchModelStatus', () => {
  beforeEach(() => {
    cmdMock.mockReset();
  });

  it('offers the download when the model is not on disk, and starts it on click', async () => {
    cmdMock.mockImplementation((name: string) =>
      Promise.resolve(
        name === 'download_embedding_model' ? status('preparing', 0, 262 * MB) : status('not_downloaded'),
      ),
    );
    render(<LocalSearchModelStatus />);
    await act(async () => {});
    expect(screen.getByTestId('local-search-model-state')).toHaveTextContent('settings.ai.localModel.notDownloaded');
    fireEvent.click(screen.getByTestId('local-search-model-download'));
    await act(async () => {});
    expect(cmdMock).toHaveBeenCalledWith('download_embedding_model');
  });

  it('shows a failure with its reason and a retry, never "ready"', async () => {
    cmdMock.mockResolvedValue(status('failed', 0, 0, 'embedding model download failed (model_fp16.onnx): HTTP 503'));
    render(<LocalSearchModelStatus />);
    await act(async () => {});
    expect(screen.getByTestId('local-search-model-error')).toHaveTextContent('HTTP 503');
    expect(screen.getByTestId('local-search-model-download')).toHaveTextContent('settings.ai.localModel.retry');
    expect(screen.queryByText('settings.ai.localModel.ready')).not.toBeInTheDocument();
  });

  it('a failed status call shows "checking", not a state it cannot see', async () => {
    cmdMock.mockRejectedValue(new Error('ipc down'));
    render(<LocalSearchModelStatus />);
    await act(async () => {});
    expect(screen.getByTestId('local-search-model-state')).toHaveTextContent('settings.ai.localModel.checking');
    expect(screen.queryByTestId('local-search-model-download')).not.toBeInTheDocument();
  });
});
