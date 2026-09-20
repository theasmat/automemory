import { create } from 'zustand';
import { wsClient, ConnectionState } from '../services/wsClient';
import type {
  Memory,
  MemoryProposal,
  SuccessTrace,
  FailureLog,
  GraphEntity,
  GraphEdge,
  Project,
  ToolEvent,
  SystemStats,
  PulseMessage,
  ImpactReport,
} from '../types';

export type TabType = 'overview' | 'graph' | 'memories' | 'proposals' | 'traces' | 'failures' | 'context' | 'events';

interface AutoMemoryStore {
  // Navigation & Filter
  activeTab: TabType;
  setActiveTab: (tab: TabType) => void;
  selectedProjectId: string | null;
  setSelectedProjectId: (id: string | null) => void;
  searchQuery: string;
  setSearchQuery: (q: string) => void;

  // Connection
  connectionState: ConnectionState;
  latencyMs: number;
  setConnectionState: (state: ConnectionState) => void;
  setLatencyMs: (ms: number) => void;

  // Data
  stats: SystemStats;
  projects: Project[];
  memories: Memory[];
  proposals: MemoryProposal[];
  traces: SuccessTrace[];
  failures: FailureLog[];
  graphEntities: GraphEntity[];
  graphEdges: GraphEdge[];
  recentEvents: ToolEvent[];

  // Context Playground
  tokenBudget: number;
  setTokenBudget: (budget: number) => void;
  assembledContext: string;
  isAssemblingContext: boolean;

  // Impact Analysis
  impactReport: ImpactReport | null;
  isTracingImpact: boolean;

  // Actions
  init: () => void;
  fetchStats: () => Promise<void>;
  fetchProjects: () => Promise<void>;
  fetchMemories: () => Promise<void>;
  fetchProposals: () => Promise<void>;
  fetchTraces: () => Promise<void>;
  fetchFailures: () => Promise<void>;
  fetchGraph: () => Promise<void>;
  fetchEvents: () => Promise<void>;
  decideProposal: (id: string, approved: boolean) => Promise<boolean>;
  triggerSync: () => Promise<void>;
  assembleContext: (query?: string) => Promise<void>;
  traceImpact: (target: string) => Promise<void>;
  handlePulse: (pulse: PulseMessage) => void;
}

export const useAutoMemoryStore = create<AutoMemoryStore>((set, get) => ({
  activeTab: 'overview',
  setActiveTab: (tab) => set({ activeTab: tab }),
  selectedProjectId: null,
  setSelectedProjectId: (id) => {
    set({ selectedProjectId: id });
    const state = get();
    state.fetchMemories();
    state.fetchProposals();
    state.fetchTraces();
    state.fetchFailures();
    state.fetchGraph();
  },
  searchQuery: '',
  setSearchQuery: (q) => {
    set({ searchQuery: q });
    get().fetchMemories();
  },

  connectionState: 'disconnected',
  latencyMs: 0,
  setConnectionState: (connectionState) => set({ connectionState }),
  setLatencyMs: (latencyMs) => set({ latencyMs }),

  stats: {
    active_memories: 0,
    archived_memories: 0,
    active_traces: 0,
    unresolved_failures: 0,
    total_projects: 0,
    pending_proposals: 0,
  },
  projects: [],
  memories: [],
  proposals: [],
  traces: [],
  failures: [],
  graphEntities: [],
  graphEdges: [],
  recentEvents: [],

  tokenBudget: 3000,
  setTokenBudget: (tokenBudget) => set({ tokenBudget }),
  assembledContext: '',
  isAssemblingContext: false,

  impactReport: null,
  isTracingImpact: false,

  init: () => {
    wsClient.onStateChange((state) => {
      set({ connectionState: state });
      if (state === 'connected') {
        const s = get();
        s.fetchStats();
        s.fetchProjects();
        s.fetchMemories();
        s.fetchProposals();
        s.fetchTraces();
        s.fetchFailures();
        s.fetchGraph();
        s.fetchEvents();
      }
    });

    wsClient.onLatency((ms) => set({ latencyMs: ms }));

    wsClient.onPulse((pulse) => {
      get().handlePulse(pulse);
    });

    wsClient.onBroadcast((msg) => {
      if (msg.type === 'proposal_decided') {
        get().fetchProposals();
        get().fetchMemories();
        get().fetchStats();
      }
    });

    wsClient.connect();
  },

  handlePulse: (pulse) => {
    set((state) => ({
      stats: {
        active_memories: pulse.active_memories,
        archived_memories: pulse.archived_memories,
        active_traces: pulse.active_traces,
        unresolved_failures: pulse.unresolved_failures,
        total_projects: state.stats.total_projects,
        pending_proposals: pulse.pending_proposals,
      },
      recentEvents: pulse.recent_events && pulse.recent_events.length > 0 ? pulse.recent_events : state.recentEvents,
    }));
  },

  fetchStats: async () => {
    try {
      const stats = await wsClient.call<SystemStats>('stats');
      set({ stats });
    } catch {
      // ignore
    }
  },

  fetchProjects: async () => {
    try {
      const projects = await wsClient.call<Project[]>('projects');
      set({ projects });
    } catch {
      // ignore
    }
  },

  fetchMemories: async () => {
    try {
      const { selectedProjectId, searchQuery } = get();
      const memories = await wsClient.call<Memory[]>('memories', {
        project_id: selectedProjectId,
        q: searchQuery || undefined,
        limit: 100,
      });
      set({ memories });
    } catch {
      // ignore
    }
  },

  fetchProposals: async () => {
    try {
      const { selectedProjectId } = get();
      const proposals = await wsClient.call<MemoryProposal[]>('proposals', {
        project_id: selectedProjectId,
      });
      set({ proposals });
    } catch {
      // ignore
    }
  },

  fetchTraces: async () => {
    try {
      const { selectedProjectId } = get();
      const traces = await wsClient.call<SuccessTrace[]>('traces', {
        project_id: selectedProjectId,
        limit: 50,
      });
      set({ traces });
    } catch {
      // ignore
    }
  },

  fetchFailures: async () => {
    try {
      const { selectedProjectId } = get();
      const failures = await wsClient.call<FailureLog[]>('failures', {
        project_id: selectedProjectId,
        limit: 50,
      });
      set({ failures });
    } catch {
      // ignore
    }
  },

  fetchGraph: async () => {
    try {
      const { selectedProjectId } = get();
      const res = await wsClient.call<{ entities: GraphEntity[]; edges: GraphEdge[] }>('graph', {
        project_id: selectedProjectId,
        limit_entities: 400,
        limit_edges: 800,
      });
      set({ graphEntities: res.entities || [], graphEdges: res.edges || [] });
    } catch {
      // ignore
    }
  },

  fetchEvents: async () => {
    try {
      const recentEvents = await wsClient.call<ToolEvent[]>('events', { limit: 50 });
      set({ recentEvents });
    } catch {
      // ignore
    }
  },

  decideProposal: async (id: string, approved: boolean) => {
    try {
      const res = await wsClient.call<{ success: boolean }>('decide_proposal', { id, approved });
      if (res.success) {
        set((state) => ({
          proposals: state.proposals.filter((p) => p.id !== id),
        }));
        get().fetchStats();
        get().fetchMemories();
        return true;
      }
      return false;
    } catch {
      return false;
    }
  },

  triggerSync: async () => {
    try {
      await wsClient.call('trigger_sync');
    } catch {
      // ignore
    }
  },

  assembleContext: async (query?: string) => {
    set({ isAssemblingContext: true });
    try {
      const { selectedProjectId, tokenBudget } = get();
      const res = await wsClient.call<{ context: string }>('assemble_context', {
        project_id: selectedProjectId,
        q: query || undefined,
        token_budget: tokenBudget,
      });
      set({ assembledContext: res.context || '', isAssemblingContext: false });
    } catch (e: any) {
      set({ assembledContext: `Failed to assemble context: ${e.message}`, isAssemblingContext: false });
    }
  },

  traceImpact: async (target: string) => {
    set({ isTracingImpact: true, impactReport: null });
    try {
      const { selectedProjectId } = get();
      const res = await wsClient.call<{ success: boolean; report?: ImpactReport; message?: string }>('trace_impact', {
        project_id: selectedProjectId,
        target,
        depth: 3,
      });
      if (res.success && res.report) {
        set({ impactReport: res.report, isTracingImpact: false });
      } else {
        set({ impactReport: null, isTracingImpact: false });
      }
    } catch {
      set({ impactReport: null, isTracingImpact: false });
    }
  },
}));
