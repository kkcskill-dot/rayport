import { useEffect, useState, type FormEvent } from 'react';
import {
  formatTimestamp,
  validatePolicy,
} from '../../application/AppsWebSrcApplicationRayportSession';
import type { Execution, Policy } from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  EmptyState,
  ErrorText,
  Field,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

function PolicyForm({
  policy,
  api,
  run,
}: Pick<WorkspacePageProps, 'api' | 'run'> & { policy: Policy }) {
  const [value, setValue] = useState(policy);
  const [error, setError] = useState('');
  async function save(event: FormEvent) {
    event.preventDefault();
    const problem = validatePolicy(value);
    setError(problem ?? '');
    if (!problem) await run(() => api.updatePolicy(value), '실행 정책을 저장했습니다.');
  }
  return (
    <form className="panel section-gap" onSubmit={(event) => void save(event)}>
      <h2>실행 예산</h2>
      <div className="form-grid">
        <Field label="사용자별 분당 실행">
          <input
            type="number"
            min={1}
            max={10000}
            required
            value={value.per_user_per_minute}
            onChange={(event) =>
              setValue({ ...value, per_user_per_minute: Number(event.target.value) })
            }
          />
        </Field>
        <Field label="원천별 분당 실행">
          <input
            type="number"
            min={1}
            max={10000}
            required
            value={value.source_per_minute}
            onChange={(event) =>
              setValue({ ...value, source_per_minute: Number(event.target.value) })
            }
          />
        </Field>
        <Field label="최대 동시 실행">
          <input
            type="number"
            min={1}
            max={10000}
            required
            value={value.max_concurrent}
            onChange={(event) => setValue({ ...value, max_concurrent: Number(event.target.value) })}
          />
        </Field>
      </div>
      <ErrorText>{error}</ErrorText>
      <button className="button primary">예산 저장</button>
    </form>
  );
}
export default function RayportOperationsPage({ api, workspace, run }: WorkspacePageProps) {
  const policy = workspace.policy;
  const metrics = workspace.operations;
  const [executions, setExecutions] = useState<Execution[] | null>(null);
  async function reloadExecutions() {
    const list = await run(() => api.executions(), '', false);
    if (list) setExecutions(list);
  }
  useEffect(() => {
    void reloadExecutions();
  }, [api, metrics.active_executions]);
  async function reconcile(id: string) {
    const result = await run(
      () => api.reconcileExecution(id),
      '원천에서 실행 종료를 확인했습니다.',
    );
    if (result?.terminated) await reloadExecutions();
  }
  return (
    <>
      <h1>운영과 정책</h1>
      <p className="subtitle">신규 실행을 통제하고 부하·변경 이력을 확인합니다.</p>
      <section className={`policy-banner ${policy.paused ? 'paused' : ''}`}>
        <div>
          <span className={`pill ${policy.paused ? 'amber' : ''}`}>
            {policy.paused ? '신규 실행 중지' : '예산 내 실행 허용'}
          </span>
          <h2>
            {policy.paused ? '새로운 조회가 차단됩니다.' : '승인된 도구만 실행할 수 있습니다.'}
          </h2>
          <p>
            실행 중 작업의 종료 여부는 별도로 확인합니다. 중지 버튼이 DB 종료를 보장하지 않습니다.
          </p>
        </div>
        <button
          className={`button ${policy.paused ? 'primary' : 'secondary danger-text'}`}
          onClick={() =>
            void run(
              () => api.updatePolicy({ ...policy, paused: !policy.paused }),
              policy.paused ? '신규 실행을 다시 허용했습니다.' : '신규 실행을 차단했습니다.',
            )
          }
        >
          {policy.paused ? '신규 실행 재개' : '신규 실행 중지'}
        </button>
      </section>
      <div className="summary-grid section-gap">
        <div className="metric">
          <span>현재 실행</span>
          <strong>{metrics.active_executions}</strong>
        </div>
        <div className="metric">
          <span>완료한 실행</span>
          <strong>{metrics.completed_executions}</strong>
        </div>
        <div className="metric">
          <span>차단한 실행</span>
          <strong>{metrics.blocked_executions}</strong>
        </div>
      </div>
      <p className="callout">
        APIC 원천 실행: {metrics.source_execution_enabled ? '연결 설정에 따라 활성화' : '비활성'}.
        마트 장애 시 운영 원천으로 자동 전환하지 않습니다. 지표는 서버가 보고한 현재 상태입니다.
      </p>
      <PolicyForm key={JSON.stringify(policy)} policy={policy} api={api} run={run} />
      <section className="panel section-gap">
        <div className="section-heading">
          <h2>실행 종료 확인</h2>
          <button className="button secondary" onClick={() => void reloadExecutions()}>
            실행 목록 갱신
          </button>
        </div>
        <p className="muted">
          종료 확인은 원천의 확인 응답을 검사합니다. 응답이 없으면 실행 슬롯과 예산을 강제로
          해제하지 않습니다.
        </p>
        {executions === null ? (
          <p role="status">실행 목록을 확인하고 있습니다.</p>
        ) : executions.length === 0 ? (
          <EmptyState title="확인할 실행이 없습니다" />
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>실행</th>
                  <th>도구 / 사용자</th>
                  <th>상태</th>
                  <th>시작</th>
                  <th>작업</th>
                </tr>
              </thead>
              <tbody>
                {executions.map((execution) => (
                  <tr key={execution.id}>
                    <td>{execution.id}</td>
                    <td>
                      {execution.tool_id} / {execution.actor}
                    </td>
                    <td>{execution.status === 'in_doubt' ? '종료 미확인' : '실행 중'}</td>
                    <td>{formatTimestamp(execution.started_at)}</td>
                    <td>
                      <button
                        className="button secondary"
                        disabled={execution.status !== 'in_doubt'}
                        onClick={() => void reconcile(execution.id)}
                      >
                        원천 종료 확인
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
      <section className="panel section-gap">
        <h2>감사 이력</h2>
        {workspace.audit.length === 0 ? (
          <EmptyState title="기록된 감사 이벤트가 없습니다" />
        ) : (
          <div className="table-scroll">
            <table>
              <caption className="sr-only">작업 시각, 사용자와 변경 대상</caption>
              <thead>
                <tr>
                  <th>시각</th>
                  <th>주체</th>
                  <th>작업</th>
                  <th>대상</th>
                </tr>
              </thead>
              <tbody>
                {workspace.audit.map((event) => (
                  <tr key={event.id}>
                    <td>{formatTimestamp(event.at)}</td>
                    <td>{event.actor}</td>
                    <td>{event.action}</td>
                    <td>{event.object_id}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </>
  );
}
