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

export interface DelegationScoreEntry {
  subject: string;
  overall_score: number;
  factors: {
    pattern_stability: number;
    security_sensitivity: number;
    codebase_complexity: number;
    decision_density: number;
    ai_track_record: number;
  };
  recommendation: string;
  caveats: string[];
}

export const createAgentSlice: StateCreator<AppStore, [], [], AgentSlice> = (set, get) => ({
  agentMemories: [],
  delegationScores: [],
  agentDataExists: false,
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

  loadDelegationScores: async () => {
    try {
      const scores = await cmd('get_all_delegation_scores') as unknown as DelegationScoreEntry[];
      set({ delegationScores: scores });
    } catch (error) {
      console.error('Failed to load delegation scores:', error);
    }
  },

  checkAgentDataExists: async () => {
    // Derive from already-loaded memories if available, avoiding a redundant IPC call
    const { agentMemories } = get();
    if (agentMemories.length > 0) {
      set({ agentDataExists: true });
      return;
    }
    try {
      const memories = await cmd('recall_agent_memories', {
        subject: '',
        limit: 1,
      }) as unknown as AgentMemoryEntry[];
      set({ agentDataExists: memories.length > 0 });
    } catch {
      set({ agentDataExists: false });
    }
  },

  promoteMemoryToDecision: async (memoryId: number) => {
    try {
      await cmd('promote_memory_to_decision', { memoryId });
      // Reload agent memories and decisions after promotion
      const memories = await cmd('recall_agent_memories', {
        subject: '',
        limit: 50,
      }) as unknown as AgentMemoryEntry[];
      set({ agentMemories: memories });
      // Also reload decisions if available
      get().loadDecisions();
    } catch (error) {
      console.error('Failed to promote memory to decision:', error);
    }
  },
});
