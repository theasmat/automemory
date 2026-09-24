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

/**
 * Low-level HTTP client helper with standardized error handling and JSON parsing.
 */
async function request<T>(endpoint: string, options?: RequestInit): Promise<T | null> {
  try {
    const res = await fetch(endpoint, options);
    if (!res.ok) {
      console.warn(`[AutoMemory API] Request to ${endpoint} failed with status ${res.status}`);
      return null;
    }
    return (await res.json()) as T;
  } catch (err) {
    console.warn(`[AutoMemory API] Network/Parse error on ${endpoint}:`, err);
    return null;
  }
}

export const api = {
  async getStats(): Promise<SystemStats | null> {
    return request<SystemStats>('/api/stats');
  },

  async getMemories(projectId?: string | null): Promise<Memory[]> {
    const url = projectId
      ? `/api/memories?project_id=${encodeURIComponent(projectId)}`
      : '/api/memories';
    return (await request<Memory[]>(url)) || [];
  },

  async getSkills(projectId?: string | null): Promise<any[]> {
    const url = projectId
      ? `/api/skills?project_id=${encodeURIComponent(projectId)}`
      : '/api/skills';
    return (await request<any[]>(url)) || [];
  },

  async getConstitution(projectId?: string | null): Promise<string> {
    const url = projectId
      ? `/api/constitution?project_id=${encodeURIComponent(projectId)}`
      : '/api/constitution';
    const data = await request<{ constitution: string }>(url);
    return data?.constitution || '';
  },

  async getSessions(projectId?: string | null): Promise<SessionSummary[]> {
    const url = projectId
      ? `/api/sessions?project_id=${encodeURIComponent(projectId)}`
      : '/api/sessions';
    return (await request<SessionSummary[]>(url)) || [];
  },

  async getSessionEvents(sessionId: string): Promise<SessionEvent[]> {
    const url = `/api/sessions/${encodeURIComponent(sessionId)}/events`;
    return (await request<SessionEvent[]>(url)) || [];
  },

  async distillSession(sessionId: string): Promise<DistillationReport | null> {
    const url = `/api/sessions/${encodeURIComponent(sessionId)}/distill`;
    return request<DistillationReport>(url, { method: 'POST' });
  },

  async getTraces(projectId?: string | null): Promise<SuccessTrace[]> {
    const url = projectId
      ? `/api/traces?project_id=${encodeURIComponent(projectId)}`
      : '/api/traces';
    return (await request<SuccessTrace[]>(url)) || [];
  },

  async getFailures(projectId?: string | null): Promise<FailureRecord[]> {
    const url = projectId
      ? `/api/failures?project_id=${encodeURIComponent(projectId)}`
      : '/api/failures';
    return (await request<FailureRecord[]>(url)) || [];
  },

  async getProposals(projectId?: string | null): Promise<Proposal[]> {
    const url = projectId
      ? `/api/proposals?project_id=${encodeURIComponent(projectId)}`
      : '/api/proposals';
    return (await request<Proposal[]>(url)) || [];
  },

  async decideProposal(id: string, approved: boolean): Promise<boolean> {
    try {
      const res = await fetch('/api/proposals/decide', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id, approved }),
      });
      return res.ok;
    } catch {
      return false;
    }
  },

  async getProjects(): Promise<Project[]> {
    return (await request<Project[]>('/api/projects')) || [];
  },

  async getMode(projectId?: string | null): Promise<string | null> {
    const url = projectId
      ? `/api/mode?project_id=${encodeURIComponent(projectId)}`
      : '/api/mode';
    const data = await request<{ mode: string }>(url);
    return data?.mode?.toLowerCase() || null;
  },

  async setMode(mode: string, project?: string | null): Promise<boolean> {
    try {
      const res = await fetch('/api/mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode, project: project || 'default' }),
      });
      return res.ok;
    } catch {
      return false;
    }
  },

  async getGuardrailRules(): Promise<any[]> {
    return (await request<any[]>('/api/guardrails/rules')) || [];
  },

  async evaluateGuardrail(payload: {
    tool_name: string;
    args: Record<string, any>;
    project_id?: string | null;
    mode?: string;
  }): Promise<any> {
    return request<any>('/api/guardrails/evaluate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },
};
