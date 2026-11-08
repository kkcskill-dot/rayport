import {
  RayportApiError,
  type RayportApi,
} from '../../application/ports/AppsWebSrcApplicationPortsRayportApi';
import type { QueryResult } from '../../domain/AppsWebSrcDomainRayportTypes';

export function createHttpRayportApi(fetcher: typeof fetch = fetch): RayportApi {
  async function request<T>(path: string, method = 'GET', payload?: unknown): Promise<T> {
    let response: Response;
    try {
      response = await fetcher(`/api/v1${path}`, {
        method,
        credentials: 'same-origin',
        cache: 'no-store',
        headers: {
          Accept: 'application/json',
          ...(method === 'GET' ? {} : { 'X-Rayport-Request': '1' }),
          ...(payload === undefined ? {} : { 'Content-Type': 'application/json' }),
        },
        ...(payload === undefined ? {} : { body: JSON.stringify(payload) }),
      });
    } catch {
      throw new RayportApiError(
        0,
        'network_error',
        '서버에 연결하지 못했습니다. 연결을 확인한 후 다시 시도하세요.',
      );
    }
    if (response.status === 204) {
      if (method === 'DELETE') return undefined as T;
      throw new RayportApiError(204, 'invalid_response', '서버 응답에 필요한 데이터가 없습니다.');
    }
    let envelope: unknown;
    try {
      envelope = await response.json();
    } catch {
      throw new RayportApiError(
        response.status,
        'invalid_response',
        '서버가 올바른 JSON 응답을 보내지 않았습니다.',
      );
    }
    if (!response.ok) {
      const error = (envelope as { error?: { code?: unknown; message?: unknown } } | null)?.error;
      const fallback =
        response.status === 401
          ? '로그인이 필요하거나 세션이 만료되었습니다.'
          : response.status === 403
            ? '이 작업을 수행할 권한이 없습니다.'
            : '요청을 처리하지 못했습니다.';
      throw new RayportApiError(
        response.status,
        typeof error?.code === 'string' ? error.code : 'request_failed',
        typeof error?.message === 'string' ? error.message : fallback,
      );
    }
    if (!envelope || typeof envelope !== 'object' || !('data' in envelope))
      throw new RayportApiError(
        response.status,
        'invalid_response',
        '서버 응답에 필요한 데이터가 없습니다.',
      );
    return (envelope as { data: T }).data;
  }
  const id = (value: string) => encodeURIComponent(value);
  return {
    session: () => request('/session'),
    login: (token) => request('/session', 'POST', { token }),
    logout: () => request('/session', 'DELETE'),
    workspace: () => request('/workspace'),
    resolve: (question) => request('/questions/resolve', 'POST', { question }),
    query: async (toolId, input) => {
      const result = await request<QueryResult>(`/tools/${id(toolId)}/query`, 'POST', input);
      const numbers = [
        result.net_minor,
        result.gross_minor,
        result.cancel_minor,
        ...result.rows.flatMap((row) => [row.gross_minor, row.cancel_minor]),
      ];
      if (!numbers.every(Number.isSafeInteger))
        throw new RayportApiError(
          502,
          'unsafe_amount',
          '금액이 브라우저의 정확한 정수 표시 범위를 초과합니다. 운영자에게 문의하세요.',
        );
      return result;
    },
    createTool: (tool) => request('/tools', 'POST', tool),
    updateTool: (toolId, tool) => request(`/tools/${id(toolId)}`, 'PUT', tool),
    validateTool: (toolId, evidence) =>
      request(`/tools/${id(toolId)}/validate`, 'POST', { evidence }),
    publishTool: (toolId) => request(`/tools/${id(toolId)}/publish`, 'POST', {}),
    retireTool: (toolId) => request(`/tools/${id(toolId)}/retire`, 'POST', {}),
    createRequest: (question, purpose) => request('/requests', 'POST', { question, purpose }),
    transitionRequest: (requestId, status, linked_tool_id) =>
      request(`/requests/${id(requestId)}/transition`, 'POST', { status, linked_tool_id }),
    importDiscovery: (input) => request('/discovery/import', 'POST', input),
    draftCandidate: (candidateId, input) =>
      request(`/candidates/${id(candidateId)}/draft`, 'POST', input),
    importMart: (input) => request('/marts/import', 'POST', input),
    updatePolicy: (policy) => request('/policy', 'PUT', policy),
    executions: () => request('/executions'),
    reconcileExecution: (executionId) =>
      request(`/executions/${id(executionId)}/reconcile`, 'POST', {}),
    toolContract: (toolId) => request(`/tools/${id(toolId)}/contract`),
  };
}
