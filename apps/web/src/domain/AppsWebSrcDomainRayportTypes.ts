/** Shared vocabulary. No UI, HTTP, or persistence dependencies. KRW amounts are integer won. */
export type RayportPage = 'ask' | 'catalog' | 'candidates' | 'marts' | 'requests' | 'operations';
export type Role = 'admin' | 'analyst' | 'viewer';
export type Principal = { id: string; tenant_id: string; role: Role; store_ids: string[] };
export type SourceKind = 'mart' | 'apic';
export type ToolStatus = 'draft' | 'validated' | 'published' | 'retired';
export type Tool = {
  id: string;
  name: string;
  description: string;
  keywords: string[];
  source: SourceKind;
  source_ref: string;
  sql_template: string | null;
  status: ToolStatus;
  version: number;
  max_days: number;
  max_rows: number;
  freshness_seconds: number;
  owner: string;
  validation_evidence: string | null;
};
export type QueryInput = { start_date: string; end_date_exclusive: string; store_ids: string[] };
export type SalesRow = {
  business_date: string;
  store_id: string;
  gross_minor: number;
  cancel_minor: number;
  currency: string;
};
export type QueryResult = {
  rows: SalesRow[];
  net_minor: number;
  gross_minor: number;
  cancel_minor: number;
  currency: string;
  dataset_version: number;
  data_as_of: string;
  source: string;
};
export type Resolution = {
  status: 'matched' | 'missing' | 'ambiguous';
  tool_id: string | null;
  explanation: string;
  suggestions: string[];
};
export type RequestStatus =
  'submitted' | 'in_review' | 'building' | 'ready' | 'resolved' | 'rejected';
export type CapabilityRequest = {
  id: string;
  question: string;
  purpose: string;
  status: RequestStatus;
  requester_id: string;
  linked_tool_id: string | null;
  created_at: string;
};
export type Candidate = {
  id: string;
  name: string;
  sql_template: string;
  signature: string;
  executions: number | null;
  ash_samples: number | null;
  module: string | null;
  converted_tool_id: string | null;
  source_label: string;
  observed_at: string;
  observation_version: number;
  window_start: string | null;
  window_end: string | null;
};
export type ToolContract = {
  tool_id: string;
  version: number;
  openapi: Record<string, unknown>;
  mcp_tool: Record<string, unknown>;
};
export type DiscoveryImport = {
  source_label: string;
  diagnostics_authorized: boolean;
  observations: {
    sql_text: string;
    executions: number | null;
    ash_samples: number | null;
    module: string | null;
  }[];
};
export type CandidateDraftInput = {
  name: string;
  source_ref: string;
  source: SourceKind;
  owner: string;
};
export type MartSummary = {
  id: string;
  name: string;
  version: number;
  rows_count: number;
  data_as_of: string;
  complete_through: string;
};
export type MartImport = {
  id: string;
  name: string;
  data_as_of: string;
  complete_through: string;
  rows: SalesRow[];
};
export type Policy = {
  paused: boolean;
  per_user_per_minute: number;
  source_per_minute: number;
  max_concurrent: number;
};
export type Execution = {
  id: string;
  actor: string;
  tool_id: string;
  source_ref: string;
  source: SourceKind;
  status: 'running' | 'in_doubt';
  started_at: number;
  charged_days: number;
};
export type AuditEvent = {
  id: string;
  at: string;
  actor: string;
  action: string;
  object_id: string;
};
export type Workspace = {
  tools: Tool[];
  candidates: Candidate[];
  requests: CapabilityRequest[];
  marts: MartSummary[];
  policy: Policy;
  audit: AuditEvent[];
  operations: {
    active_executions: number;
    blocked_executions: number;
    completed_executions: number;
    source_execution_enabled: boolean;
  };
};
