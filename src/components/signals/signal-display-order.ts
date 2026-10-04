// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// What the Signal list is SHOWING, in on-screen order, plus the lane
// disclosure state. Keyboard shortcuts (j/k move, s save, d dismiss, o open,
// Enter expand) index into this list, so the item an action hits is the item
// that is highlighted. Before this existed the shortcuts indexed the raw,
// unsorted relevanceResults while the highlight walked the display order —
// "s" could save a different item from the one on screen.

import { create } from 'zustand';
import type { SourceRelevance } from '../../types';

interface SignalDisplayOrderState {
  /** Visible rows in display order; null when the Signal list is not mounted. */
  visible: SourceRelevance[] | null;
  stackExpanded: boolean;
  moreExpanded: boolean;
  setVisible: (visible: SourceRelevance[] | null) => void;
  setStackExpanded: (v: boolean) => void;
  setMoreExpanded: (v: boolean) => void;
}

export const useSignalDisplayOrder = create<SignalDisplayOrderState>((set) => ({
  visible: null,
  stackExpanded: false,
  moreExpanded: false,
  setVisible: (visible) => set({ visible }),
  setStackExpanded: (stackExpanded) => set({ stackExpanded }),
  setMoreExpanded: (moreExpanded) => set({ moreExpanded }),
}));
