export type MemoryType = 'fact' | 'decision' | 'lesson' | 'pattern' | 'warning' | 'observation' | 'preference' | 'skill' | string;
export type MemoryStatus = 'active' | 'superseded' | 'archived' | 'invalid' | 'review' | string;

export interface Memory {
  id: string;
  project_id: string | null;
  type: MemoryType;
  content: string;
  summary?: string;
  confidence: number;
  importance?: number;
  freshness?: number;
  status: MemoryStatus;
  version?: number | null;
  superseded_by?: string | null;
  tags?: string[];
  entities?: string[];
  created_at: string;
  updated_at: string;
}

export interface TraceStep {
  position?: number;
  action: string;
  tool?: string;
  command?: string | null;
  verification?: string | null;
}

export interface SuccessTrace {
  id: string;
  project_id: string | null;
  title: string;
  category?: string;
  status: string;
  preconditions?: string[];
  steps?: TraceStep[];
  reproducibility_score: number;
  caveats?: string[];
  source_task_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface FailureLog {
  id: string;
  project_id: string | null;
  task_title?: string;
  task_description?: string;
  category?: string | null;
  failed_tool?: string | null;
  error_message: string;
  root_cause_hypothesis?: string | null;
  root_cause?: string | null;
  fix_applied?: string | null;
  verification_result?: string | null;
  status: string;
  created_at: string;
  updated_at?: string;
}

export interface MemoryProposal {
  id: string;
  project_id: string | null;
  type?: string;
  proposal_type?: string;
  source_type?: string;
  target_id?: string;
  description?: string;
  proposed_content?: string;
  suggested_action?: string;
  confidence?: number;
  status: string;
  created_at: string;
  decided_at?: string | null;
}

export interface GraphEntity {
  id: string;
  project_id: string | null;
  entity_type: 'file' | 'function' | 'struct' | 'class' | 'doc' | 'memory_cluster' | string;
  name: string;
  path: string;
  line_start: number | null;
  line_end: number | null;
  metadata?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface GraphEdge {
  source_id: string;
  target_id: string;
  relation_type: 'defines' | 'imports' | 'calls' | 'references' | 'modifies' | string;
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

export interface ToolInfo {
  name: string;
  category?: string;
  input?: unknown;
  output?: unknown;
  status?: string;
  duration_ms?: number;
}

export interface AgentInfo {
  adapter_id: string;
  agent_instance_id?: string;
}

export interface ToolEvent {
  event_id?: string;
  id?: string;
  event_type?: string;
  timestamp?: string;
  created_at?: string;
  agent?: AgentInfo;
  session_id?: string | null;
  task_id?: string | null;
  tool?: ToolInfo;
  tool_name?: string;
  payload?: Record<string, unknown>;
  raw_payload?: unknown;
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
  recent_events?: ToolEvent[];
  timestamp: string;
}

export interface WsRpcResponse<T = unknown> {
  id: string | number | null;
  result?: T;
  error?: string | null;
}
