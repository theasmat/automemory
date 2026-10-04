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
    proposalsSummary: null as any,
    projects: [] as Project[],
    activeProject: null as string | null,
    currentMode: 'vibe' as 'vibe' | 'strict' | 'review',
    guardrailRules: [] as any[],
    docs: [] as any[],
    syncStatus: null as any,

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
          this.fetchDocs(),
          this.fetchSyncStatus(),
        ]);
      } finally {
        this.isLoading = false;
      }
    },

    async fetchDocs() {
      this.docs = await api.getDocs(this.activeProject);
    },

    async fetchSyncStatus() {
      this.syncStatus = await api.getSyncStatus(this.activeProject);
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
        this.fetchProposalsSummary();
      }
    },

    async fetchProposalsSummary() {
      const summary = await api.getProposalsSummary(this.activeProject);
      if (summary) {
        this.proposalsSummary = summary;
      }
      return summary;
    },

    async decideAllProposals(approved: boolean, proposalType?: string | null) {
      this.isLoading = true;
      const res = await api.decideAllProposals(approved, proposalType, this.activeProject);
      if (res && res.success) {
        const action = approved ? 'Approved' : 'Rejected';
        this.showLiveToast(
          `${action} ${res.result.decided_count} proposals (${res.result.archived_memories_count} duplicates archived)`,
          approved ? 'success' : 'info'
        );
        await Promise.allSettled([
          this.fetchProposals(),
          this.fetchProposalsSummary(),
          this.fetchStats(),
          this.fetchMemories(),
        ]);
      } else {
        this.showLiveToast('Failed to execute bulk proposal decision', 'warning');
      }
      this.isLoading = false;
      return res;
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

    async archiveMemory(id: string) {
      const ok = await api.archiveMemory(id);
      if (ok) {
        this.memories = this.memories.filter((m) => m.id !== id);
        this.stats.active_memories = Math.max(0, this.stats.active_memories - 1);
        this.stats.archived_memories += 1;
        this.showLiveToast('Memory archived successfully', 'success');
      }
      return ok;
    },

    async updateMemory(id: string, payload: { summary?: string; content?: string; reason?: string }) {
      const ok = await api.updateMemory(id, payload);
      if (ok) {
        const mem = this.memories.find((m) => m.id === id);
        if (mem) {
          if (payload.summary) mem.summary = payload.summary;
          if (payload.content) mem.content = payload.content;
        }
        this.showLiveToast('Memory updated successfully', 'success');
      }
      return ok;
    },

    async resolveFailure(id: string, fixApplied: string, verificationResult?: string) {
      const ok = await api.resolveFailure(id, {
        fix_applied: fixApplied,
        verification_result: verificationResult,
      });
      if (ok) {
        const f = this.failures.find((item) => item.id === id);
        if (f) {
          f.status = 'resolved';
          f.fix_applied = fixApplied;
          if (verificationResult) f.verification_result = verificationResult;
        }
        this.stats.unresolved_failures = Math.max(0, this.stats.unresolved_failures - 1);
        this.showLiveToast('Failure resolved and playbook recorded', 'success');
      }
      return ok;
    },

    async addGuardrailRule(rule: {
      name: string;
      pattern: string;
      category?: string;
      is_critical?: boolean;
      description?: string;
      mitigation_hint?: string;
    }) {
      const ok = await api.addGuardrailRule({
        ...rule,
        project_id: this.activeProject,
      });
      if (ok) {
        this.showLiveToast(`Custom guardrail rule '${rule.name}' activated`, 'success');
        await this.fetchGuardrailRules();
      }
      return ok;
    },

    async deleteGuardrailRule(id: string) {
      const ok = await api.deleteGuardrailRule(id);
      if (ok) {
        this.guardrailRules = this.guardrailRules.filter((r) => r.id !== id);
        this.showLiveToast('Custom guardrail rule deleted', 'info');
      }
      return ok;
    },

    async toggleGuardrailRule(id: string, enabled: boolean) {
      const ok = await api.toggleGuardrailRule(id, enabled);
      if (ok) {
        const r = this.guardrailRules.find((rule) => rule.id === id);
        if (r) r.enabled = enabled;
        this.showLiveToast(`Rule ${enabled ? 'enabled' : 'disabled'}`, 'info');
      }
      return ok;
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
