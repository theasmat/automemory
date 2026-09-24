import { defineStore } from 'pinia';
import type {
  Memory,
  SessionSummary,
  SessionEvent,
  DistillationReport,
  SuccessTrace,
  FailureRecord,
  Proposal,
  Project,
  SystemStats,
} from '~/types';
import { api } from '~/utils/api';

let wsClient: ReturnType<typeof useWsPulse> | null = null;

export const useAutoMemoryStore = defineStore('automemory', {
  state: () => ({
    stats: {
      active_memories: 0,
      archived_memories: 0,
      active_traces: 0,
      unresolved_failures: 0,
      pending_proposals: 0,
      total_projects: 0,
      total_sessions: 0,
    } as SystemStats,

    memories: [] as Memory[],
    skills: [] as any[],
    constitution: '' as string,
    sessions: [] as SessionSummary[],
    activeSessionId: null as string | null,
    sessionEvents: [] as SessionEvent[],
    distillationReport: null as DistillationReport | null,
    traces: [] as SuccessTrace[],
    failures: [] as FailureRecord[],
    proposals: [] as Proposal[],
    projects: [] as Project[],
    activeProject: null as string | null,
    currentMode: 'vibe' as 'vibe' | 'strict' | 'review',
    guardrailRules: [] as any[],

    // UI state
    isConnected: false,
    isDistilling: false,
    isLoading: false,
    scopeFilter: 'all' as 'all' | 'vault' | 'cloud',
    memorySearch: '',
    liveToast: null as { message: string; type: string; timestamp: number } | null,
  }),

  getters: {
    filteredMemories(state): Memory[] {
      let list = state.memories;

      // Scope filter
      if (state.scopeFilter === 'vault') {
        list = list.filter((m) => m.scope === 'project' || m.source_kind === 'vault');
      } else if (state.scopeFilter === 'cloud') {
        list = list.filter((m) => m.scope === 'global' || m.source_kind === 'cloud');
      }

      // Search query filter
      if (state.memorySearch.trim()) {
        const q = state.memorySearch.toLowerCase();
        list = list.filter(
          (m) =>
            m.content.toLowerCase().includes(q) ||
            m.summary.toLowerCase().includes(q) ||
            m.type.toLowerCase().includes(q) ||
            m.tags.some((t) => t.toLowerCase().includes(q))
        );
      }

      return list;
    },

    activeSession(state): SessionSummary | undefined {
      return state.sessions.find((s) => s.session_id === state.activeSessionId);
    },
  },

  actions: {
    showLiveToast(message: string, type: 'info' | 'success' | 'warning' = 'info') {
      this.liveToast = { message, type, timestamp: Date.now() };
      setTimeout(() => {
        if (this.liveToast && Date.now() - this.liveToast.timestamp >= 3800) {
          this.liveToast = null;
        }
      }, 4000);
    },

    async fetchAll() {
      this.isLoading = true;
      try {
        await Promise.allSettled([
          this.fetchStats(),
          this.fetchMode(),
          this.fetchGuardrailRules(),
          this.fetchMemories(),
          this.fetchSkills(),
          this.fetchConstitution(),
          this.fetchSessions(),
          this.fetchTraces(),
          this.fetchFailures(),
          this.fetchProposals(),
          this.fetchProjects(),
        ]);
      } finally {
        this.isLoading = false;
      }
    },

    async fetchStats() {
      const data = await api.getStats();
      if (data) {
        this.stats = {
          active_memories: data.active_memories ?? 0,
          archived_memories: data.archived_memories ?? 0,
          active_traces: data.active_traces ?? 0,
          unresolved_failures: data.unresolved_failures ?? 0,
          pending_proposals: data.pending_proposals ?? 0,
          total_projects: data.total_projects ?? 0,
          total_sessions: data.total_sessions ?? 0,
        };
      }
    },

    async fetchMemories() {
      this.memories = await api.getMemories(this.activeProject);
    },

    async fetchSkills() {
      this.skills = await api.getSkills(this.activeProject);
    },

    async fetchConstitution() {
      this.constitution = await api.getConstitution(this.activeProject);
    },

    async fetchSessions() {
      this.sessions = await api.getSessions(this.activeProject);
      if (this.sessions.length > 0 && !this.activeSessionId) {
        this.selectSession(this.sessions[0].session_id);
      }
    },

    async selectSession(sessionId: string) {
      this.activeSessionId = sessionId;
      this.distillationReport = null;
      this.sessionEvents = await api.getSessionEvents(sessionId);
    },

    async distillSession(sessionId: string) {
      this.isDistilling = true;
      try {
        const report = await api.distillSession(sessionId);
        if (report) {
          this.distillationReport = report;
          this.showLiveToast('100-to-1 Trajectory Distillation synthesized successfully', 'success');
        }
      } finally {
        this.isDistilling = false;
      }
    },

    async fetchTraces() {
      this.traces = await api.getTraces(this.activeProject);
    },

    async fetchFailures() {
      this.failures = await api.getFailures(this.activeProject);
    },

    async fetchProposals() {
      this.proposals = await api.getProposals(this.activeProject);
    },

    async fetchProjects() {
      this.projects = await api.getProjects();
    },

    async decideProposal(id: string, approved: boolean) {
      const ok = await api.decideProposal(id, approved);
      if (ok) {
        this.proposals = this.proposals.filter((p) => p.id !== id);
        this.stats.pending_proposals = Math.max(0, this.stats.pending_proposals - 1);
        this.showLiveToast(
          `Proposal ${approved ? 'approved' : 'rejected'} successfully`,
          approved ? 'success' : 'warning'
        );
        this.fetchMemories();
      }
    },

    async fetchMode() {
      const mode = await api.getMode(this.activeProject);
      if (mode && (mode === 'vibe' || mode === 'strict' || mode === 'review')) {
        this.currentMode = mode;
      }
    },

    async setMode(mode: 'vibe' | 'strict' | 'review', project?: string) {
      const ok = await api.setMode(mode, project || this.activeProject);
      if (ok) {
        this.currentMode = mode;
        this.showLiveToast(`Autopilot mode switched to ${mode.toUpperCase()}`, 'success');
      }
    },

    async fetchGuardrailRules() {
      this.guardrailRules = await api.getGuardrailRules();
    },

    async evaluateGuardrail(tool_name: string, args: Record<string, any>) {
      return api.evaluateGuardrail({
        tool_name,
        args,
        project_id: this.activeProject,
        mode: this.currentMode,
      });
    },

    // ── WebSocket Real-Time Engine ──────────────────────────────────────────
    initWebSocket() {
      if (import.meta.server) return;
      if (wsClient) return;

      wsClient = useWsPulse();
      wsClient.connect();

      wsClient.onMessage((msg) => {
        this.handleIncomingWsMessage(msg);
      });

      setInterval(() => {
        if (wsClient) {
          this.isConnected = wsClient.isConnected.value;
        }
      }, 500);
    },

    sendWsRpc(method: string, params: Record<string, any> = {}) {
      if (wsClient) {
        wsClient.sendRpc(method, params);
      }
    },

    handleIncomingWsMessage(msg: any) {
      // 1. Handle RPC responses
      if (msg.result) {
        if (msg.result.active_memories !== undefined) {
          this.stats = { ...this.stats, ...msg.result };
        } else if (Array.isArray(msg.result)) {
          if (msg.result.length > 0 && msg.result[0].content !== undefined) {
            this.memories = msg.result;
          } else if (msg.result.length > 0 && msg.result[0].session_id !== undefined) {
            this.sessions = msg.result;
          }
        }
        return;
      }

      // 2. Handle Real-Time Event Broadcasts
      if (msg.event_type) {
        const evType = msg.event_type as string;
        this.showLiveToast(`⚡ Live event: ${evType}`, 'info');

        if (msg.session_id && msg.session_id === this.activeSessionId) {
          this.sessionEvents.push({
            event_id: msg.event_id || `ev-${Date.now()}`,
            session_id: msg.session_id,
            event_type: evType,
            timestamp: msg.timestamp || new Date().toISOString(),
            adapter_id: msg.agent?.adapter_id || 'agy',
            tool_name: msg.tool?.tool_name || null,
            tool_args: msg.tool?.input_preview || null,
            tool_output: msg.tool?.output_preview || null,
            is_error: msg.tool?.is_error ?? false,
            raw_payload: msg.raw_payload,
          });
        }

        if (evType.includes('memory') || evType.includes('save')) {
          this.fetchMemories();
          this.fetchStats();
        } else if (evType.includes('failure') || evType.includes('error')) {
          this.fetchFailures();
          this.fetchStats();
        } else if (evType.includes('trace')) {
          this.fetchTraces();
          this.fetchSkills();
          this.fetchStats();
        } else if (evType.includes('proposal')) {
          this.fetchProposals();
          this.fetchStats();
        } else {
          this.fetchSessions();
          this.fetchStats();
        }
      }
    },
  },
});
