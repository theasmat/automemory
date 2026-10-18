export type MemoryType =
  | 'fact'
  | 'decision'
  | 'lesson'
  | 'pattern'
  | 'warning'
  | 'observation'
  | 'preference'
  | 'skill';

export type MemoryStatus =
  | 'active'
  | 'superseded'
  | 'archived'
  | 'invalid'
  | 'review';

export interface Memory {
  id: string;
  scope: 'global' | 'project';
  project_id?: string | null;
  type: MemoryType;
  content: string;
  summary: string;
  confidence: number;
  importance: number;
  freshness: number;
  status: MemoryStatus;
  source_kind: string;
  version?: number;
  previous_version_id?: string | null;
  created_at: string;
  updated_at: string;
  last_recalled_at?: string | null;
  tags: string[];
  entities: string[];
  metadata?: Record<string, any>;
}

export interface SessionSummary {
  session_id: string;
  project_id?: string | null;
  adapter_id: string;
  event_count: number;
  error_count: number;
  first_event_at: string;
  last_event_at: string;
}

export interface SessionEvent {
  event_id: string;
  session_id: string;
  event_type: string;
  timestamp: string;
  adapter_id: string;
  tool_name?: string | null;
  tool_args?: string | null;
  tool_output?: string | null;
  is_error: boolean;
  raw_payload?: Record<string, any>;
}

export interface DistillationStep {
  step_index: number;
  action_name: string;
  summary: string;
  diff_preview?: string | null;
  command?: string | null;
  output_snippet?: string | null;
  is_essential: boolean;
  is_verification: boolean;
}

export interface DistillationReport {
  session_id: string;
  total_events: number;
  distilled_events: number;
  noise_pruned_percentage: number;
  golden_steps: DistillationStep[];
  lessons_learned: string[];
  unresolved_errors: string[];
  distilled_at: string;
}

export interface SuccessTrace {
  id: string;
  project_id?: string | null;
  task_intent: string;
  golden_steps: Array<Record<string, any>>;
  verification_command?: string | null;
  tags: string[];
  created_at: string;
  updated_at: string;
}

export interface FailureRecord {
  id: string;
  project_id?: string | null;
  task_title: string;
  error_message: string;
  failed_tool?: string | null;
  root_cause_hypothesis?: string | null;
  status: 'unresolved' | 'resolved';
  fix_applied?: string | null;
  verification_result?: string | null;
  created_at: string;
  updated_at: string;
}

export interface Proposal {
  id: string;
  project_id?: string | null;
  proposal_type: string;
  target_memory_id?: string | null;
  proposed_content?: string | null;
  proposed_summary?: string | null;
  reason: string;
  status: 'pending' | 'approved' | 'rejected';
  confidence: number;
  created_at: string;
  type?: string;
  description?: string;
}

export interface Project {
  id: string;
  slug: string;
  display_name: string;
  created_at: string;
  updated_at: string;
}

export interface SystemStats {
  active_memories: number;
  archived_memories: number;
  total_memory_tokens?: number;
  total_event_tokens?: number;
  tokens_saved_estimate?: number;
  active_traces: number;
  unresolved_failures: number;
  pending_proposals: number;
  total_projects: number;
  total_sessions: number;
}

export interface Step {
  id: string;
  title: string;
  done: boolean;
  why: string;
  command: string;
  link: string;
}

export interface OnboardingState {
  completed_steps: string[];
  dismissed: boolean;
  dismissed_flags: Record<string, boolean>;
  demo_loaded: boolean;
  tour_step: number;
}

export interface LearnTopic {
  id: string;
  title: string;
  summary: string;
  content: string;
}

