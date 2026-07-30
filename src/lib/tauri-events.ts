// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import { listen, type EventCallback, type UnlistenFn } from '@tauri-apps/api/event';

import { hasTauriRuntime } from './tauri-runtime';

export function safeListen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (!import.meta.env.VITEST && !hasTauriRuntime()) {
    return Promise.resolve(() => {});
  }
  return listen<T>(event, handler).catch(() => () => {});
}

export type { UnlistenFn };
