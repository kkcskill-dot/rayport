import type {
  Candidate,
  CandidateDraftInput,
  CapabilityRequest,
  DiscoveryImport,
  Execution,
  MartImport,
  MartSummary,
  Policy,
  Principal,
  QueryInput,
  QueryResult,
  RequestStatus,
  Resolution,
  Tool,
  ToolContract,
  Workspace,
} from '../../domain/AppsWebSrcDomainRayportTypes';

/** The application owns this port. HTTP is assembled only at bootstrap. */
export interface RayportApi {
  session(): Promise<Principal>;
  login(token: string): Promise<Principal>;
  logout(): Promise<void>;
  workspace(): Promise<Workspace>;
  resolve(question: string): Promise<Resolution>;
  query(id: string, input: QueryInput): Promise<QueryResult>;
  createTool(tool: Tool): Promise<Tool>;
  updateTool(id: string, tool: Tool): Promise<Tool>;
  toolContract(id: string): Promise<ToolContract>;
  validateTool(id: string, evidence: string): Promise<Tool>;
  publishTool(id: string): Promise<Tool>;
  retireTool(id: string): Promise<Tool>;
  createRequest(question: string, purpose: string): Promise<CapabilityRequest>;
  transitionRequest(
    id: string,
    status: RequestStatus,
    linkedToolId: string | null,
  ): Promise<CapabilityRequest>;
  importDiscovery(input: DiscoveryImport): Promise<Candidate[]>;
  draftCandidate(id: string, input: CandidateDraftInput): Promise<Tool>;
  importMart(input: MartImport): Promise<MartSummary>;
  updatePolicy(policy: Policy): Promise<Policy>;
  executions(): Promise<Execution[]>;
  reconcileExecution(id: string): Promise<{ terminated: boolean }>;
}
export class RayportApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = 'RayportApiError';
  }
}
