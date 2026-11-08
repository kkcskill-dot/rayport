import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import RayportApp from '../src/presentation/AppsWebSrcPresentationRayportApp';
import {
  RayportApiError,
  type RayportApi,
} from '../src/application/ports/AppsWebSrcApplicationPortsRayportApi';
import type { Principal, Tool, Workspace } from '../src/domain/AppsWebSrcDomainRayportTypes';

const principal: Principal = {
  id: 'worker',
  tenant_id: 'local',
  role: 'viewer',
  store_ids: ['S01'],
};
const tool: Tool = {
  id: 'sales.store',
  name: '점포별 매출',
  description: '합성 검증 데이터의 순매출',
  keywords: ['매출'],
  source: 'mart',
  source_ref: 'sample',
  sql_template: null,
  status: 'published',
  version: 1,
  max_days: 31,
  max_rows: 1000,
  freshness_seconds: 86400,
  owner: 'owner',
  validation_evidence: '확인됨',
};
function fakeApi(identity = principal) {
  const workspace: Workspace = {
    tools: [tool],
    candidates: [],
    requests: [],
    marts: [],
    policy: { paused: false, per_user_per_minute: 10, source_per_minute: 100, max_concurrent: 3 },
    audit: [],
    operations: {
      active_executions: 0,
      completed_executions: 0,
      blocked_executions: 0,
      source_execution_enabled: false,
    },
  };
  const api: RayportApi = {
    session: vi.fn(async () => identity),
    login: vi.fn(async () => identity),
    logout: vi.fn(async () => {}),
    workspace: vi.fn(async () => workspace),
    resolve: vi.fn(async () => ({
      status: 'missing' as const,
      tool_id: null,
      explanation: '현재 도구에 상품군 차원이 없습니다.',
      suggestions: [],
    })),
    query: vi.fn(async () => ({
      rows: [
        {
          business_date: '2026-11-07',
          store_id: 'S01',
          gross_minor: 120000,
          cancel_minor: 20000,
          currency: 'KRW',
        },
      ],
      gross_minor: 120000,
      cancel_minor: 20000,
      net_minor: 100000,
      currency: 'KRW',
      dataset_version: 1,
      data_as_of: '2026-11-08T00:00:00Z',
      source: 'mart',
    })),
    toolContract: vi.fn(async () => ({ tool_id: tool.id, version: 1, openapi: {}, mcp_tool: {} })),
    createTool: vi.fn(async (input) => input),
    updateTool: vi.fn(async (_id, input) => input),
    validateTool: vi.fn(async () => tool),
    publishTool: vi.fn(async () => tool),
    retireTool: vi.fn(async () => tool),
    createRequest: vi.fn(async (question, purpose) => {
      const request = {
        id: 'req-1',
        question,
        purpose,
        status: 'submitted' as const,
        requester_id: identity.id,
        linked_tool_id: null,
        created_at: '2026-11-08T00:00:00Z',
      };
      workspace.requests.push(request);
      return request;
    }),
    transitionRequest: vi.fn(async () => workspace.requests[0]),
    importDiscovery: vi.fn(async () => []),
    draftCandidate: vi.fn(async () => tool),
    importMart: vi.fn(async () => ({
      id: 'sample',
      name: '합성',
      version: 1,
      rows_count: 2,
      data_as_of: '2026-11-08T00:00:00Z',
      complete_through: '2026-11-07',
    })),
    updatePolicy: vi.fn(async (value) => value),
    executions: vi.fn(async () => []),
    reconcileExecution: vi.fn(async () => ({ terminated: true })),
  };
  return { api, workspace };
}
afterEach(cleanup);
describe('authenticated user journeys (in-process DOM, no server)', () => {
  it('shows a real login form and never invents a data session', async () => {
    const { api } = fakeApi();
    vi.mocked(api.session).mockRejectedValue(new RayportApiError(401, 'unauthorized', 'login'));
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 공간 로그인' });
    expect(screen.queryByText('12.8억')).toBeNull();
    expect(api.workspace).not.toHaveBeenCalled();
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('접근 토큰'), 'opaque-private');
    await user.click(screen.getByRole('button', { name: '로그인' }));
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    expect(api.login).toHaveBeenCalledWith('opaque-private');
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });
  it('hides administrative navigation from viewers', async () => {
    render(<RayportApp api={fakeApi().api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    expect(screen.queryByRole('button', { name: /API 발굴/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /운영과 정책/ })).toBeNull();
  });
  it('submits unsupported questions as persisted requests', async () => {
    const { api } = fakeApi();
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.type(screen.getByLabelText('어떤 데이터가 필요한가요?'), '상품군별 매출');
    await user.click(screen.getByRole('button', { name: '도구 찾기' }));
    await screen.findByRole('heading', { name: '추가 기능이 필요합니다' });
    await user.type(screen.getByLabelText('요청 목적과 필요한 항목'), '상품군 실적 비교');
    await user.click(screen.getByRole('button', { name: '생성 요청 접수' }));
    await screen.findByRole('heading', { name: '생성 요청' });
    expect(api.createRequest).toHaveBeenCalledWith('상품군별 매출', '상품군 실적 비교');
    expect(screen.getByText('상품군 실적 비교')).toBeTruthy();
  });
  it('queries explicit dates, shows actual results, and clears stale results on condition changes', async () => {
    const { api } = fakeApi();
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText('업무 도구'), tool.id);
    fireEvent.change(screen.getByLabelText('시작일'), { target: { value: '2026-11-07' } });
    fireEvent.change(screen.getByLabelText('종료일 (당일 미포함)'), {
      target: { value: '2026-11-08' },
    });
    await user.click(screen.getByRole('button', { name: '권한과 범위를 확인하고 조회' }));
    const result = await screen.findByLabelText('조회 결과');
    expect(api.query).toHaveBeenCalledWith(tool.id, {
      start_date: '2026-11-07',
      end_date_exclusive: '2026-11-08',
      store_ids: [],
    });
    expect(within(result).getAllByText('100,000원').length).toBeGreaterThan(0);
    fireEvent.change(screen.getByLabelText('시작일'), { target: { value: '2026-11-06' } });
    expect(screen.queryByLabelText('조회 결과')).toBeNull();
  });
  it('returns to login on expired session and removes workspace data', async () => {
    const { api } = fakeApi();
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    vi.mocked(api.workspace).mockRejectedValue(
      new RayportApiError(401, 'expired', '세션이 만료되었습니다.'),
    );
    await userEvent.click(screen.getByRole('button', { name: '새로고침' }));
    await screen.findByRole('heading', { name: '업무 공간 로그인' });
    expect(screen.queryByLabelText('업무 도구')).toBeNull();
  });
  it('does not treat API readiness as solved until the requester confirms it', async () => {
    const { api, workspace } = fakeApi();
    workspace.requests.push({
      id: 'req-ready',
      question: '어제 점포 매출',
      purpose: '실적 확인',
      status: 'ready',
      requester_id: 'worker',
      linked_tool_id: tool.id,
      created_at: '2026-11-07T00:00:00Z',
    });
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    await userEvent.click(screen.getByRole('button', { name: /생성 요청/ }));
    const button = screen.getByRole('button', {
      name: '해결 확인 상태로 변경',
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    await userEvent.click(
      screen.getByRole('checkbox', { name: '결과를 확인했고 원래 질문이 해결되었습니다.' }),
    );
    expect(button.disabled).toBe(false);
    await userEvent.click(screen.getByRole('button', { name: '원래 질문 다시 열기' }));
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    expect((screen.getByLabelText('어떤 데이터가 필요한가요?') as HTMLInputElement).value).toBe(
      '어제 점포 매출',
    );
    expect(api.query).not.toHaveBeenCalled();
  });
  it('shows server-side permission errors without reporting a successful mutation', async () => {
    const { api } = fakeApi({ ...principal, role: 'admin' });
    vi.mocked(api.updatePolicy).mockRejectedValue(
      new RayportApiError(403, 'forbidden', '관리자 권한을 확인하세요.'),
    );
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    await userEvent.click(screen.getByRole('button', { name: /운영과 정책/ }));
    await userEvent.click(screen.getByRole('button', { name: '신규 실행 중지' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('관리자 권한'));
    expect(screen.queryByText('신규 실행을 차단했습니다.')).toBeNull();
  });
  it('edits only unpublished drafts and sends their optimistic version', async () => {
    const { api, workspace } = fakeApi({ ...principal, role: 'admin' });
    workspace.tools = [{ ...tool, status: 'validated', version: 3 }];
    vi.mocked(api.updateTool).mockRejectedValue(
      new RayportApiError(
        409,
        'version_conflict',
        '다른 사용자가 변경했습니다. 최신 버전을 확인하세요.',
      ),
    );
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /업무 API/ }));
    await user.click(screen.getByRole('button', { name: '초안 수정' }));
    expect((screen.getByLabelText('도구 ID') as HTMLInputElement).readOnly).toBe(true);
    await user.clear(screen.getByLabelText('업무 이름'));
    await user.type(screen.getByLabelText('업무 이름'), '확인된 점포 매출');
    await user.click(screen.getByRole('button', { name: '초안 저장' }));
    await waitFor(() =>
      expect(api.updateTool).toHaveBeenCalledWith(
        tool.id,
        expect.objectContaining({ version: 3, name: '확인된 점포 매출' }),
      ),
    );
    expect(screen.getByRole('alert').textContent).toContain('다른 사용자');
    expect((screen.getByLabelText('업무 이름') as HTMLInputElement).value).toBe('확인된 점포 매출');
  });
  it('requires validation evidence before showing publication for a draft', async () => {
    const { api, workspace } = fakeApi({ ...principal, role: 'admin' });
    workspace.tools = [{ ...tool, status: 'draft', validation_evidence: null }];
    vi.mocked(api.validateTool).mockImplementation(async () => {
      workspace.tools[0] = { ...workspace.tools[0], status: 'validated' };
      return workspace.tools[0];
    });
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /업무 API/ }));
    expect(screen.queryByRole('button', { name: '검증된 버전 게시' })).toBeNull();
    expect(
      (screen.getByRole('button', { name: '검증 기록 제출' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    await user.type(
      screen.getByLabelText(/점포별 매출 검증 근거/),
      '원천과 금액 대사 및 점포 권한 확인 완료',
    );
    await user.click(screen.getByRole('button', { name: '검증 기록 제출' }));
    await screen.findByRole('button', { name: '검증된 버전 게시' });
    expect(api.publishTool).not.toHaveBeenCalled();
  });
  it('requires deliberate complete-snapshot replacement before overwriting a mart', async () => {
    const { api, workspace } = fakeApi({ ...principal, role: 'admin' });
    workspace.marts = [
      {
        id: 'sample',
        name: '기존 마트',
        version: 1,
        rows_count: 1,
        data_as_of: '2026-11-08T00:00:00Z',
        complete_through: '2026-11-07',
      },
    ];
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /데이터 마트/ }));
    fireEvent.change(screen.getByLabelText('반입할 JSON'), {
      target: { value: JSON.stringify({ id: 'sample', rows: [] }) },
    });
    await user.click(screen.getByRole('button', { name: '검증 후 새 버전 게시' }));
    expect(api.importMart).not.toHaveBeenCalled();
    expect(screen.getByRole('alert').textContent).toContain('전체 스냅샷 교체');
    await user.click(screen.getByRole('checkbox'));
    await user.click(screen.getByRole('button', { name: '검증 후 새 버전 게시' }));
    await waitFor(() => expect(api.importMart).toHaveBeenCalledTimes(1));
  });
  it('keeps uncertain executions reserved when termination cannot be confirmed', async () => {
    const { api } = fakeApi({ ...principal, role: 'admin' });
    vi.mocked(api.executions).mockResolvedValue([
      {
        id: 'exec-1',
        actor: 'worker',
        tool_id: tool.id,
        source_ref: 'apic-sales',
        source: 'apic',
        status: 'in_doubt',
        started_at: Date.parse('2026-11-08T00:00:00Z') / 1000,
        charged_days: 1,
      },
    ]);
    vi.mocked(api.reconcileExecution).mockRejectedValue(
      new RayportApiError(409, 'termination_unconfirmed', '원천에서 종료를 확인하지 못했습니다.'),
    );
    render(<RayportApp api={api} />);
    await screen.findByRole('heading', { name: '업무 데이터 질문' });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /운영과 정책/ }));
    const confirm = await screen.findByRole('button', { name: '원천 종료 확인' });
    await user.click(confirm);
    await waitFor(() =>
      expect(screen.getByRole('alert').textContent).toContain('종료를 확인하지 못했습니다'),
    );
    expect(screen.getByText('종료 미확인')).toBeTruthy();
    expect(screen.queryByText('원천에서 실행 종료를 확인했습니다.')).toBeNull();
  });
});
