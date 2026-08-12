// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, AgentSlice } from './types';

export interface AgentMemoryEntry {
  id: number;
  session_id: string;
  agent_type: string;
  memory_type: string;
  subject: string;
  content: string;
  context_tags: string[];
  created_at: string;
  expires_at: string | null;
  promoted_to_decision_id: number | null;
}

export const createAgentSlice: StateCreator<AppStore, [], [], AgentSlice> = (set) => ({
  agentMemories: [],
  agentMemoryLoading: false,

  loadAgentMemories: async () => {
    set({ agentMemoryLoading: true });
    try {
      const memories = await cmd('recall_agent_memories', {
        subject: '',
        limit: 50,
      }) as unknown as AgentMemoryEntry[];
      set({ agentMemories: memories, agentMemoryLoading: false });
    } catch {
      set({ agentMemoryLoading: false });
    }
  },

});
