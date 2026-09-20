export type MemoryType = 'decision' | 'pattern' | 'fix' | 'gotcha' | 'preference' | 'architecture';
export type MemoryStatus = 'active' | 'archived' | 'superseded';

export interface Memory {
  id: string;
  project_id: string | null;
  type: MemoryType;
  content: string;
  confidence: number;
  status: MemoryStatus;
  version: number;
  superseded_by: string | null;
  created_at: string;
  updated_at: string;
  metadata?: Record<string, unknown>;
  rrf_score?: number;
}

export interface TraceStep {
  step_number: number;
  description: string;
  tool_call?: string;
  output_summary?: string;
  timestamp: string;
}

export interface SuccessTrace {
  id: string;
  project_id: string | null;
  task_description: string;
  steps_json: string;
  tools_used: string[];
  duration_ms: number;
  reproducibility_score: number;
  created_at: string;
}

export interface FailureLog {
  id: string;
  project_id: string | null;
  task_description: string;
  error_message: string;
  root_cause: string | null;
  fix_applied: string | null;
  status: 'unresolved' | 'resolved' | 'ignored';
  created_at: string;
  resolved_at: string | null;
}

export interface MemoryProposal {
  id: string;
  project_id: string | null;
  source_type: string;
  source_id: string | null;
  type: MemoryType;
  proposed_content: string;
  confidence: number;
  status: 'pending' | 'approved' | 'rejected';
  created_at: string;
  decided_at: string | null;
}

export interface GraphEntity {
  id: string;
  project_id: string | null;
  entity_type: 'file' | 'function' | 'struct' | 'class' | 'doc' | 'memory_cluster';
  name: string;
  path: string;
  line_start: number | null;
  line_end: number | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface GraphEdge {
  source_id: string;
  target_id: string;
  relation_type: 'defines' | 'imports' | 'calls' | 'references' | 'modifies';
  confidence: number;
  created_at: string;
}

export interface ImpactReport {
  target_entity: GraphEntity;
  affected_entities: GraphEntity[];
  affected_files: string[];
  depth: number;
}

export interface Project {
  id: string;
  slug: string;
  name: string;
  root_path: string;
  created_at: string;
  updated_at: string;
}

export interface Preference {
  id: string;
  project_id: string | null;
  key: string;
  value: string;
  scope: 'global' | 'project';
  locked: boolean;
  created_at: string;
  updated_at: string;
}

export interface ToolEvent {
  id: string;
  session_id: string | null;
  tool_name: string;
  payload: Record<string, unknown>;
  created_at: string;
}

export interface SystemStats {
  active_memories: number;
  archived_memories: number;
  active_traces: number;
  unresolved_failures: number;
  total_projects: number;
  pending_proposals: number;
}

export interface PulseMessage {
  type: 'pulse';
  active_memories: number;
  archived_memories: number;
  active_traces: number;
  unresolved_failures: number;
  pending_proposals: number;
  recent_events: ToolEvent[];
  timestamp: string;
}

export interface WsRpcResponse<T = unknown> {
  id: string | number | null;
  result?: T;
  error?: string | null;
}
