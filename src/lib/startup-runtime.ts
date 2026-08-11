// SPDX-License-Identifier: FSL-1.1-Apache-2.0

import { cmd } from './commands';

let victauriDogfoodMode: Promise<boolean> | null = null;

export function isVictauriDogfoodMode(): Promise<boolean> {
  victauriDogfoodMode ??= cmd('get_startup_runtime_flags')
    .then(flags => flags.victauriE2e)
    .catch(() => false);
  return victauriDogfoodMode;
}
