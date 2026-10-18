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
import { usePasswordModal } from '~/composables/usePasswordModal';

export interface ApiResponse<T> {
  ok: boolean;
  data: T | null;
  error?: string;
}

/**
 * Standardized HTTP request helper returning { ok, data, error }.
 * Never swallows outages into silent empty lists.
 */
export async function request<T>(endpoint: string, options?: RequestInit): Promise<ApiResponse<T>> {
  try {
    const res = await fetch(endpoint, options);
    if (!res.ok) {
      let errText = '';
      try {
        const json = await res.json();
        errText = json.error || json.message || '';
      } catch {
        errText = await res.text().catch(() => '');
      }
      return {
        ok: false,
        data: null,
        error: errText || `Request failed with HTTP ${res.status}`,
      };
    }
    const text = await res.text();
    if (!text || !text.trim()) {
      return { ok: true, data: null };
    }
    try {
      const data = JSON.parse(text) as T;
      return { ok: true, data };
    } catch {
      return { ok: true, data: text as unknown as T };
    }
  } catch (err: any) {
    return {
      ok: false,
      data: null,
      error: err?.message || 'AutoMemory daemon is unreachable',
    };
  }
}

export const apiGet = request;

/**
 * fetch for safety-gated writes. When a human password is set the server answers 403;
 * prompts using the masked PasswordModal, remembers for this tab (sessionStorage) and retries.
 */
export async function humanFetch(
  endpoint: string,
  actionName: string = 'this action',
  options: RequestInit = {}
): Promise<Response> {
  const withPassword = (password: string | null): RequestInit => ({
    ...options,
    headers: {
      ...(options.headers as Record<string, string>),
      ...(password ? { 'X-AutoMemory-Password': password } : {}),
    },
  });

  let password: string | null = null;
  try {
    password = sessionStorage.getItem('am_human_password');
  } catch {
    /* storage blocked */
  }

  let res = await fetch(endpoint, withPassword(password));
  const needsPassword = async (r: Response) =>
    r.status === 403 && /password/i.test(await r.clone().text().catch(() => ''));

  if (await needsPassword(res)) {
    const { promptPassword, setError } = usePasswordModal();
    const entered = await promptPassword(actionName);
    if (entered) {
      try {
        sessionStorage.setItem('am_human_password', entered);
      } catch {
        /* storage blocked */
      }
      res = await fetch(endpoint, withPassword(entered));
      if (await needsPassword(res)) {
        try {
          sessionStorage.removeItem('am_human_password');
        } catch {
          /* storage blocked */
        }
        setError('Wrong password. Please try again.');
      }
    }
  }

  return res;
}

export const api = {
  async getStats(): Promise<ApiResponse<SystemStats>> {
    return request<SystemStats>('/api/stats');
  },

  async getCoordination(projectId?: string | null): Promise<ApiResponse<any>> {
    const url = projectId
      ? `/api/coordination?project_id=${encodeURIComponent(projectId)}`
      : '/api/coordination';
    return request<any>(url);
  },

  async getMemories(projectId?: string | null): Promise<ApiResponse<Memory[]>> {
    const url = projectId
      ? `/api/memories?project_id=${encodeURIComponent(projectId)}`
      : '/api/memories';
    return request<Memory[]>(url);
  },

  async addMemory(payload: { summary: string; content?: string; type?: string; project_id?: string | null }): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/memories', 'add a memory', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error || 'Failed to add memory' };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async updateMemory(id: string, payload: { summary?: string; content?: string; reason?: string; pinned?: boolean }): Promise<ApiResponse<any>> {
    try {
      const res = await fetch(`/api/memories/${encodeURIComponent(id)}/update`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async archiveMemory(id: string): Promise<ApiResponse<any>> {
    try {
      const res = await fetch(`/api/memories/${encodeURIComponent(id)}/archive`, {
        method: 'POST',
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getContext(projectId?: string | null, query?: string): Promise<string> {
    const url = `/api/context?${new URLSearchParams({
      ...(projectId ? { project_id: projectId } : {}),
      ...(query ? { q: query } : {}),
    })}`;
    try {
      const res = await fetch(url);
      return await res.text();
    } catch {
      return '';
    }
  },

  async getProposals(projectId?: string | null): Promise<ApiResponse<Proposal[]>> {
    const url = projectId
      ? `/api/proposals?project_id=${encodeURIComponent(projectId)}`
      : '/api/proposals';
    return request<Proposal[]>(url);
  },

  async getProposalsSummary(projectId?: string | null): Promise<ApiResponse<any>> {
    const url = projectId
      ? `/api/proposals/summary?project_id=${encodeURIComponent(projectId)}`
      : '/api/proposals/summary';
    return request<any>(url);
  },

  async decideProposal(id: string, approved: boolean, reason?: string): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/proposals/decide', approved ? 'approve suggestion' : 'reject suggestion', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id, approved, reason }),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async decideAllProposals(approved: boolean, proposalType?: string | null, projectId?: string | null): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/proposals/decide_all', approved ? 'approve all suggestions' : 'reject all suggestions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ approved, proposal_type: proposalType, project_id: projectId }),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getMode(projectId?: string | null): Promise<string | null> {
    const url = projectId
      ? `/api/mode?project_id=${encodeURIComponent(projectId)}`
      : '/api/mode';
    const res = await request<{ mode: string }>(url);
    return res.data?.mode?.toLowerCase() || null;
  },

  async setMode(mode: string, project?: string | null): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/mode', `switch safety mode to ${mode}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode, project: project || 'default' }),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getGuardrailRules(): Promise<ApiResponse<any[]>> {
    return request<any[]>('/api/guardrails/rules');
  },

  async addGuardrailRule(payload: {
    name: string;
    pattern: string;
    category?: string;
    is_critical?: boolean;
    description?: string;
    mitigation_hint?: string;
    project_id?: string | null;
  }): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/guardrails/rules', 'add guardrail rule', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async deleteGuardrailRule(id: string): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch(`/api/guardrails/rules/${encodeURIComponent(id)}`, 'delete guardrail rule', {
        method: 'DELETE',
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async toggleGuardrailRule(id: string, enabled: boolean): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch(`/api/guardrails/rules/${encodeURIComponent(id)}/toggle`, 'toggle guardrail rule', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async evaluateGuardrail(payload: {
    tool_name: string;
    args: Record<string, any>;
    project_id?: string | null;
    mode?: string;
  }): Promise<ApiResponse<any>> {
    return request<any>('/api/guardrails/evaluate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },

  async getEvents(): Promise<ApiResponse<any[]>> {
    return request<any[]>('/api/events');
  },

  async getProjects(): Promise<ApiResponse<Project[]>> {
    return request<Project[]>('/api/projects');
  },

  async getSyncStatus(projectId?: string | null): Promise<ApiResponse<any>> {
    const url = projectId
      ? `/api/sync/status?project_id=${encodeURIComponent(projectId)}`
      : '/api/sync/status';
    return request<any>(url);
  },

  async triggerSync(): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/sync/trigger', 'sync repository', {
        method: 'POST',
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getDocs(projectId?: string | null): Promise<ApiResponse<any[]>> {
    const url = projectId
      ? `/api/docs?project_id=${encodeURIComponent(projectId)}`
      : '/api/docs';
    return request<any[]>(url);
  },

  async getConstitution(projectId?: string | null): Promise<string> {
    const url = projectId
      ? `/api/constitution?project_id=${encodeURIComponent(projectId)}`
      : '/api/constitution';
    const res = await request<{ constitution: string }>(url);
    return res.data?.constitution || '';
  },

  async getSkills(projectId?: string | null): Promise<ApiResponse<any[]>> {
    const url = projectId
      ? `/api/skills?project_id=${encodeURIComponent(projectId)}`
      : '/api/skills';
    return request<any[]>(url);
  },

  async getTraces(projectId?: string | null): Promise<ApiResponse<SuccessTrace[]>> {
    const url = projectId
      ? `/api/traces?project_id=${encodeURIComponent(projectId)}`
      : '/api/traces';
    return request<SuccessTrace[]>(url);
  },

  async getFailures(projectId?: string | null): Promise<ApiResponse<FailureRecord[]>> {
    const url = projectId
      ? `/api/failures?project_id=${encodeURIComponent(projectId)}`
      : '/api/failures';
    return request<FailureRecord[]>(url);
  },

  async resolveFailure(id: string, payload: { fix_applied: string; verification_result?: string }): Promise<ApiResponse<any>> {
    try {
      const res = await fetch(`/api/failures/${encodeURIComponent(id)}/resolve`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getSessions(projectId?: string | null): Promise<ApiResponse<SessionSummary[]>> {
    const url = projectId
      ? `/api/sessions?project_id=${encodeURIComponent(projectId)}`
      : '/api/sessions';
    return request<SessionSummary[]>(url);
  },

  async getSessionEvents(sessionId: string): Promise<ApiResponse<SessionEvent[]>> {
    const url = `/api/sessions/${encodeURIComponent(sessionId)}/events`;
    return request<SessionEvent[]>(url);
  },

  async distillSession(sessionId: string): Promise<ApiResponse<DistillationReport>> {
    const url = `/api/sessions/${encodeURIComponent(sessionId)}/distill`;
    return request<DistillationReport>(url, { method: 'POST' });
  },

  async getGraph(projectId?: string | null): Promise<ApiResponse<any>> {
    const url = projectId
      ? `/api/graph?project_id=${encodeURIComponent(projectId)}`
      : '/api/graph';
    return request<any>(url);
  },

  async getSetup(): Promise<ApiResponse<Step[]>> {
    return request<Step[]>('/api/setup');
  },

  async getOnboarding(): Promise<ApiResponse<OnboardingState>> {
    return request<OnboardingState>('/api/onboarding');
  },

  async updateOnboarding(payload: Record<string, any>): Promise<ApiResponse<OnboardingState>> {
    try {
      const res = await humanFetch('/api/onboarding', 'update onboarding progress', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async seedDemo(): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/demo', 'seed sample demo project', {
        method: 'POST',
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async removeDemo(): Promise<ApiResponse<any>> {
    try {
      const res = await humanFetch('/api/demo', 'remove sample demo project', {
        method: 'DELETE',
      });
      const data = await res.json().catch(() => ({}));
      return { ok: res.ok, data, error: res.ok ? undefined : data.error };
    } catch (err: any) {
      return { ok: false, data: null, error: err.message };
    }
  },

  async getLearnTopics(): Promise<ApiResponse<LearnTopic[]>> {
    return request<LearnTopic[]>('/api/learn');
  },
};
