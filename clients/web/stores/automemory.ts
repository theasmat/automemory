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
  Step,
  OnboardingState,
  LearnTopic,
} from '~/types';
import { api } from '~/utils/api';
import { useWsPulse } from '~/composables/useWsPulse';
import { useToast } from '~/composables/useToast';

let wsClient: ReturnType<typeof useWsPulse> | null = null;

/** Events carry the agent as an object ({adapter_id}), leases as a string; return a lowercase name for matching. */
export function agentLabel(a: any): string {
  if (typeof a === 'string') return a.toLowerCase();
  return String(a?.adapter_id ?? a?.name ?? '').toLowerCase();
}

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
    currentMode: 'review' as 'vibe' | 'strict' | 'review',
    guardrailRules: [] as any[],
    docs: [] as any[],
    syncStatus: null as any,
    coordination: {
      leases: [] as any[],
      rooms: [] as any[],
      batons: [] as any[],
      alerts: [] as any[],
    },
    recentEvents: [] as any[],
    contextPreview: '' as string,

    // UI state
    isConnected: false,
    isDistilling: false,
    isLoading: false,
    daemonError: null as string | null,
    memorySearch: '',

    // Onboarding & Guided Tour State
    setupSteps: [] as Step[],
    onboardingState: {
      completed_steps: [],
      dismissed: false,
      dismissed_flags: {},
      demo_loaded: false,
      tour_step: 0,
    } as OnboardingState,
    learnTopics: [] as LearnTopic[],
    daemonOutdated: false,
    isTourActive: false,
    tourKind: 'intro' as 'intro' | 'showcase',
    isHelpDrawerOpen: false,
    roomVersion: 0,
  }),

  getters: {
    filteredMemories(state): Memory[] {
      let list = state.memories;
      if (state.memorySearch.trim()) {
        const q = state.memorySearch.toLowerCase();
        list = list.filter(
          (m) =>
            m.content.toLowerCase().includes(q) ||
            m.summary.toLowerCase().includes(q) ||
            m.type.toLowerCase().includes(q) ||
            m.tags?.some((t) => t.toLowerCase().includes(q))
        );
      }
      return list;
    },

    activeSession(state): SessionSummary | undefined {
      return state.sessions.find((s) => s.session_id === state.activeSessionId);
    },

    activeLeases(state): any[] {
      return state.coordination.leases || [];
    },

    /** The id of the only real (non-sample) project, or null when there are none or several. */
    soleProjectId(state): string | null {
      const real = state.projects.filter((p: any) => p.id !== 'demo');
      return real.length === 1 ? real[0].id : null;
    },

    connectedAgents(state): any[] {
      const known = [
        { id: 'claude', name: 'Claude Code' },
        { id: 'cursor', name: 'Cursor' },
        { id: 'codex', name: 'Codex' },
        { id: 'agy', name: 'Antigravity' },
      ];

      const leases = state.coordination.leases || [];
      const events = state.recentEvents || [];
      const now = Date.now();

      return known.map((a) => {
        const heldLeases = leases.filter(
          (l: any) => agentLabel(l.agent).includes(a.id) || agentLabel(l.agent).includes(a.name.toLowerCase())
        );
        const lastEv = events.find(
          (e: any) => agentLabel(e.agent).includes(a.id) || agentLabel(e.agent).includes(a.name.toLowerCase())
        );

        let status = 'not connected';
        if (heldLeases.length > 0) {
          status = 'editing';
        } else if (lastEv && now - new Date(lastEv.timestamp || lastEv.created_at).getTime() < 10 * 60 * 1000) {
          status = 'active';
        } else if (lastEv || state.stats.active_memories > 0) {
          status = 'idle';
        }

        return {
          ...a,
          status,
          locks: heldLeases.map((l: any) => l.scope),
          last_seen: lastEv ? lastEv.timestamp || lastEv.created_at : undefined,
        };
      });
    },

    setupChecklist(state): Step[] {
      if (state.setupSteps && state.setupSteps.length > 0) {
        return state.setupSteps;
      }
      return [
        {
          id: 'agent',
          title: 'Agent connected',
          done: false,
          why: 'Connect an agent like Claude Code, Cursor or Antigravity to share memory.',
          command: 'automemory connect agy',
          link: '/agents',
        },
        {
          id: 'instruction_files',
          title: 'Instruction files',
          done: false,
          why: 'Generate instruction blocks (AGENTS.md) so agents read shared rules.',
          command: 'automemory export-rules',
          link: '/brain',
        },
        {
          id: 'password',
          title: 'Password set',
          done: false,
          why: 'Set a human password to protect approvals, rules, and modes.',
          command: 'automemory rule passwd',
          link: '/safety',
        },
        {
          id: 'sync',
          title: 'Sync to another machine',
          done: false,
          why: 'Set up an encrypted git remote vault to sync memories across devices.',
          command: 'automemory sync remote <url>',
          link: '/sync',
        },
      ];
    },

    isDemoMode(state): boolean {
      return state.onboardingState.demo_loaded || state.activeProject === 'demo';
    },

    nextSetupStep(state): Step | undefined {
      const list = state.setupSteps.length > 0 ? state.setupSteps : [];
      return list.find((s) => !s.done);
    },
  },

  actions: {
    async fetchAll() {
      this.isLoading = true;
      try {
        await this.fetchStats();
        await Promise.allSettled([
          this.fetchMode(),
          this.fetchCoordination(),
          this.fetchEvents(),
          this.fetchContextPreview(),
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
          this.fetchSetup(),
          this.fetchOnboarding(),
          this.fetchLearnTopics(),
        ]);
      } finally {
        this.isLoading = false;
      }
    },

    async fetchSetup() {
      const res = await api.getSetup();
      if (res.ok && Array.isArray(res.data)) {
        this.setupSteps = res.data;
        this.daemonOutdated = false;
      } else if (res.error?.includes('older than this dashboard')) {
        this.daemonOutdated = true;
      }
    },

    async fetchOnboarding() {
      const res = await api.getOnboarding();
      if (res.ok && res.data && typeof res.data === 'object') {
        this.onboardingState = { ...this.onboardingState, ...res.data, dismissed_flags: res.data.dismissed_flags ?? {}, completed_steps: res.data.completed_steps ?? [] };
      }
    },

    async fetchLearnTopics() {
      const res = await api.getLearnTopics();
      if (res.ok && Array.isArray(res.data)) {
        this.learnTopics = res.data;
      }
    },

    async updateOnboardingProgress(payload: Record<string, any>) {
      const res = await api.updateOnboarding(payload);
      if (res.ok && res.data && typeof res.data === 'object') {
        this.onboardingState = { ...this.onboardingState, ...res.data, dismissed_flags: res.data.dismissed_flags ?? {}, completed_steps: res.data.completed_steps ?? [] };
      }
      return res.ok;
    },

    async seedDemoProject() {
      const { success, error } = useToast();
      const res = await api.seedDemo();
      if (res.ok) {
        success('Sample demo project loaded! Exploring with sample data.');
        this.activeProject = 'demo';
        await this.fetchAll();
      } else {
        error(res.error || 'Failed to seed sample project');
      }
      return res.ok;
    },

    async removeDemoProject() {
      const { success, error } = useToast();
      const res = await api.removeDemo();
      if (res.ok) {
        success('Sample demo project removed.');
        if (this.activeProject === 'demo') {
          this.activeProject = null;
        }
        await this.fetchAll();
      } else {
        error(res.error || 'Failed to remove sample project');
      }
      return res.ok;
    },

    /** 'intro' is the short first tour; 'showcase' walks every page using the sample data. */
    startTour(kind: 'intro' | 'showcase' = 'intro') {
      this.tourKind = kind;
      this.isTourActive = true;
    },

    stopTour() {
      this.isTourActive = false;
    },

    openHelpDrawer() {
      this.isHelpDrawerOpen = true;
    },

    closeHelpDrawer() {
      this.isHelpDrawerOpen = false;
    },

    async fetchStats() {
      const res = await api.getStats();
      if (res.ok && res.data) {
        this.daemonError = null;
        this.isConnected = true;
        this.stats = {
          active_memories: res.data.active_memories ?? 0,
          archived_memories: res.data.archived_memories ?? 0,
          active_traces: res.data.active_traces ?? 0,
          unresolved_failures: res.data.unresolved_failures ?? 0,
          pending_proposals: res.data.pending_proposals ?? 0,
          total_projects: res.data.total_projects ?? 0,
          total_sessions: res.data.total_sessions ?? 0,
        };
      } else {
        this.daemonError = res.error || "AutoMemory daemon is unreachable";
        this.isConnected = false;
      }
    },

    async fetchCoordination() {
      const res = await api.getCoordination(this.activeProject);
      if (res.ok && res.data) {
        this.daemonError = null;
        this.coordination = {
          leases: Array.isArray(res.data.leases) ? res.data.leases : [],
          rooms: Array.isArray(res.data.rooms) ? res.data.rooms : [],
          batons: Array.isArray(res.data.batons) ? res.data.batons : [],
          alerts: Array.isArray(res.data.alerts) ? res.data.alerts : [],
        };
      } else if (!res.ok && res.error) {
        this.daemonError = res.error;
      }
    },

    async fetchEvents() {
      const res = await api.getEvents();
      if (res.ok && res.data) {
        this.recentEvents = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchContextPreview() {
      this.contextPreview = await api.getContext(this.activeProject);
    },

    async fetchDocs() {
      const res = await api.getDocs(this.activeProject);
      if (res.ok && res.data) {
        this.docs = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchSyncStatus() {
      const res = await api.getSyncStatus(this.activeProject);
      if (res.ok && res.data) {
        this.syncStatus = res.data;
      }
    },

    async fetchMemories() {
      const res = await api.getMemories(this.activeProject);
      if (res.ok && res.data) {
        this.daemonError = null;
        this.memories = Array.isArray(res.data) ? res.data : [];
      } else if (!res.ok) {
        this.daemonError = res.error || 'Failed to fetch memories';
      }
    },

    async fetchSkills() {
      const res = await api.getSkills(this.activeProject);
      if (res.ok && res.data) {
        this.skills = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchConstitution() {
      this.constitution = await api.getConstitution(this.activeProject);
    },

    async fetchSessions() {
      const res = await api.getSessions(this.activeProject);
      if (res.ok && res.data) {
        this.sessions = Array.isArray(res.data) ? res.data : [];
        if (this.sessions.length > 0 && !this.activeSessionId) {
          this.selectSession(this.sessions[0].session_id);
        }
      }
    },

    async selectSession(sessionId: string) {
      this.activeSessionId = sessionId;
      this.distillationReport = null;
      const res = await api.getSessionEvents(sessionId);
      if (res.ok && res.data) {
        this.sessionEvents = Array.isArray(res.data) ? res.data : [];
      }
    },

    async distillSession(sessionId: string) {
      this.isDistilling = true;
      try {
        const res = await api.distillSession(sessionId);
        if (res.ok && res.data) {
          this.distillationReport = res.data;
          const { success } = useToast();
          success('Trajectory distillation complete');
        }
      } finally {
        this.isDistilling = false;
      }
    },

    async fetchTraces() {
      const res = await api.getTraces(this.activeProject);
      if (res.ok && res.data) {
        this.traces = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchFailures() {
      const res = await api.getFailures(this.activeProject);
      if (res.ok && res.data) {
        this.failures = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchProposals() {
      const res = await api.getProposals(this.activeProject);
      if (res.ok && res.data) {
        this.proposals = Array.isArray(res.data) ? res.data : [];
      }
    },

    async fetchProjects() {
      const res = await api.getProjects();
      if (res.ok && res.data) {
        this.projects = Array.isArray(res.data) ? res.data : [];
        // "All projects" has no mode of its own; with a single real project show that project's mode.
        if (!this.activeProject && this.soleProjectId) await this.fetchMode();
      }
    },


    async decideProposal(id: string, approved: boolean, reason?: string) {
      const res = await api.decideProposal(id, approved, reason);
      const { success, error } = useToast();
      if (res.ok) {
        this.proposals = this.proposals.filter((p) => p.id !== id);
        this.stats.pending_proposals = Math.max(0, this.stats.pending_proposals - 1);
        success(approved ? 'Suggestion approved' : 'Suggestion rejected');
        this.fetchMemories();
        this.fetchProposalsSummary();
      } else {
        error(res.error || 'Failed to decide proposal');
      }
      return res.ok;
    },

    async fetchProposalsSummary() {
      const res = await api.getProposalsSummary(this.activeProject);
      if (res.ok && res.data) {
        this.proposalsSummary = res.data;
      }
      return res.data;
    },

    async decideAllProposals(approved: boolean, proposalType?: string | null) {
      this.isLoading = true;
      const res = await api.decideAllProposals(approved, proposalType, this.activeProject);
      const { success, error } = useToast();
      if (res.ok && res.data) {
        const count = res.data.result?.decided_count || 'all';
        success(`${approved ? 'Approved' : 'Rejected'} ${count} suggestions`);
        await Promise.allSettled([
          this.fetchProposals(),
          this.fetchProposalsSummary(),
          this.fetchStats(),
          this.fetchMemories(),
        ]);
      } else {
        error(res.error || 'Failed to execute bulk decision');
      }
      this.isLoading = false;
      return res.data;
    },

    async fetchMode() {
      const mode = await api.getMode(this.activeProject ?? this.soleProjectId);
      if (mode && (mode === 'vibe' || mode === 'strict' || mode === 'review')) {
        this.currentMode = mode;
      }
    },

    async setMode(mode: 'vibe' | 'strict' | 'review', project?: string) {
      const res = await api.setMode(mode, project || this.activeProject || this.soleProjectId);
      const { success, error } = useToast();
      if (res.ok) {
        this.currentMode = mode;
        success(`Safety mode set to ${mode === 'strict' ? 'Strict' : mode === 'review' ? 'Balanced' : 'Relaxed'}`);
      } else {
        error(res.error || 'Failed to switch mode');
      }
      return res.ok;
    },

    async fetchGuardrailRules() {
      const res = await api.getGuardrailRules();
      if (res.ok && res.data) {
        this.guardrailRules = res.data;
      }
    },

    async archiveMemory(id: string) {
      const mem = this.memories.find((m) => m.id === id);
      const res = await api.archiveMemory(id);
      const { success, error } = useToast();
      if (res.ok) {
        this.memories = this.memories.filter((m) => m.id !== id);
        this.stats.active_memories = Math.max(0, this.stats.active_memories - 1);
        this.stats.archived_memories += 1;

        // Undo action restores memory
        success(
          'Archived. Agents will stop using this.',
          async () => {
            if (mem) {
              await api.restoreMemory(id);
              await this.fetchMemories();
            }
          },
          'Undo'
        );
      } else {
        error(res.error || 'Failed to archive note');
      }
      return res.ok;
    },

    async addMemory(payload: { summary: string; content?: string; type?: string }) {
      const res = await api.addMemory({
        ...payload,
        project_id: this.activeProject,
      });
      const { success, error } = useToast();
      if (res.ok) {
        await this.fetchMemories();
        await this.fetchStats();
        success('Added to Project brain. Every agent will see it.');
      } else {
        error(res.error || 'Failed to add note');
      }
      return res.ok;
    },

    async updateMemory(id: string, payload: { summary?: string; content?: string; reason?: string; pinned?: boolean }) {
      const res = await api.updateMemory(id, payload);
      const { success, error } = useToast();
      if (res.ok) {
        const mem = this.memories.find((m) => m.id === id);
        if (mem) {
          if (payload.summary) mem.summary = payload.summary;
          if (payload.content) mem.content = payload.content;
          if (payload.pinned !== undefined) mem.status = payload.pinned ? 'pinned' : 'active';
        }
        success('Note updated');
      } else {
        error(res.error || 'Failed to update note');
      }
      return res.ok;
    },

    async togglePin(memory: Memory) {
      const isPinned = memory.status === 'pinned';
      return this.updateMemory(memory.id, {
        pinned: !isPinned,
        reason: isPinned ? 'Unpinned by human' : 'Pinned by human',
      });
    },

    async resolveFailure(id: string, fixApplied: string, verificationResult?: string) {
      const res = await api.resolveFailure(id, {
        fix_applied: fixApplied,
        verification_result: verificationResult,
      });
      const { success, error } = useToast();
      if (res.ok) {
        const f = this.failures.find((item) => item.id === id);
        if (f) {
          f.status = 'resolved';
          f.fix_applied = fixApplied;
          if (verificationResult) f.verification_result = verificationResult;
        }
        this.stats.unresolved_failures = Math.max(0, this.stats.unresolved_failures - 1);
        success('Resolved and saved to playbook');
      } else {
        error(res.error || 'Failed to resolve failure');
      }
      return res.ok;
    },

    async addGuardrailRule(rule: {
      name: string;
      pattern: string;
      category?: string;
      is_critical?: boolean;
      description?: string;
      mitigation_hint?: string;
    }) {
      const res = await api.addGuardrailRule({
        ...rule,
        project_id: this.activeProject,
      });
      const { success, error } = useToast();
      if (res.ok) {
        success(`Guardrail rule '${rule.name}' saved`);
        await this.fetchGuardrailRules();
      } else {
        error(res.error || 'Failed to add rule');
      }
      return res.ok;
    },

    async deleteGuardrailRule(id: string) {
      const rule = this.guardrailRules.find((r) => r.id === id);
      const res = await api.deleteGuardrailRule(id);
      const { success, error } = useToast();
      if (res.ok) {
        this.guardrailRules = this.guardrailRules.filter((r) => r.id !== id);
        success(
          'Rule deleted',
          async () => {
            if (rule) {
              await this.addGuardrailRule(rule);
            }
          },
          'Undo'
        );
      } else {
        error(res.error || 'Failed to delete rule');
      }
      return res.ok;
    },

    async toggleGuardrailRule(id: string, enabled: boolean) {
      const res = await api.toggleGuardrailRule(id, enabled);
      const { success, error } = useToast();
      if (res.ok) {
        const r = this.guardrailRules.find((rule) => rule.id === id);
        if (r) r.enabled = enabled;
        success(`Rule ${enabled ? 'enabled' : 'disabled'}`);
      } else {
        error(res.error || 'Failed to toggle rule');
      }
      return res.ok;
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
        if (wsClient && wsClient.isConnected.value) {
          this.isConnected = true;
          this.daemonError = null;
        } else if (this.daemonError !== null) {
          this.isConnected = false;
        }
      }, 500);
    },

    handleIncomingWsMessage(msg: any) {
      if (msg.type === 'room_updated') {
        this.roomVersion += 1;
        this.fetchCoordination();
        return;
      }

      if (msg.type === 'setup_changed' || msg.type === 'memory_added' || msg.type === 'lock_alert') {
        this.fetchSetup();
        this.fetchOnboarding();
        this.fetchStats();
      }

      if (msg.result) {
        if (msg.result.active_memories !== undefined) {
          this.stats = { ...this.stats, ...msg.result };
        } else if (Array.isArray(msg.result)) {
          if (msg.result.length === 0 || msg.result[0].content !== undefined) {
            this.memories = msg.result;
          } else if (msg.result[0].session_id !== undefined) {
            this.sessions = msg.result;
          }
        }
        return;
      }

      if (msg.event_type) {
        const evType = msg.event_type as string;
        if (evType.includes('setup') || evType.includes('onboarding')) {
          this.fetchSetup();
          this.fetchOnboarding();
          this.fetchStats();
        } else if (evType.includes('memory') || evType.includes('save')) {
          this.fetchMemories();
          this.fetchStats();
          this.fetchSetup();
        } else if (evType.includes('lease') || evType.includes('lock')) {
          this.fetchCoordination();
          this.fetchSetup();
        } else if (evType.includes('proposal')) {
          this.fetchProposals();
          this.fetchStats();
          this.fetchSetup();
        } else {
          this.fetchCoordination();
          this.fetchEvents();
        }
      }
    },
  },
});
