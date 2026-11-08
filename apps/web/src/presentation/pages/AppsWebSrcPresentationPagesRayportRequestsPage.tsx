import { useState } from 'react';
import {
  canManage,
  nextRequestStates,
  requestStatusLabels,
} from '../../application/AppsWebSrcApplicationRayportSession';
import type { CapabilityRequest, RequestStatus } from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  EmptyState,
  Field,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

function RequestCard({
  request,
  onReopen,
  ...props
}: WorkspacePageProps & { request: CapabilityRequest; onReopen: (question: string) => void }) {
  const admin = canManage(props.principal);
  const isRequester = request.requester_id === props.principal.id;
  const next = nextRequestStates(request.status, admin, isRequester);
  const [linked, setLinked] = useState(request.linked_tool_id ?? '');
  const [confirmSolved, setConfirmSolved] = useState(false);
  async function transition(status: RequestStatus) {
    await props.run(
      () => props.api.transitionRequest(request.id, status, linked || null),
      `요청을 '${requestStatusLabels[status]}' 상태로 변경했습니다.`,
    );
  }
  return (
    <article className="panel section-gap">
      <div className="section-heading">
        <h2>{request.question}</h2>
        <span className="pill neutral">{requestStatusLabels[request.status]}</span>
      </div>
      <p>{request.purpose}</p>
      <p className="muted">
        {request.id} · 요청자 {request.requester_id} · {request.created_at}
      </p>
      {admin && !['resolved', 'rejected'].includes(request.status) && (
        <Field label="해결에 사용할 게시 도구">
          <select value={linked} onChange={(event) => setLinked(event.target.value)}>
            <option value="">게시 도구 선택</option>
            {props.workspace.tools
              .filter((tool) => tool.status === 'published')
              .map((tool) => (
                <option key={tool.id} value={tool.id}>
                  {tool.name}
                </option>
              ))}
          </select>
        </Field>
      )}
      {request.linked_tool_id && <p className="callout">연결된 도구: {request.linked_tool_id}</p>}
      {request.status === 'ready' && (
        <>
          <p>
            원래 질문을 다시 열고 기간·점포 조건을 확인해 분석하세요. ‘어제’ 같은 상대 날짜는 현재
            기준으로 자동 실행하지 않습니다.
          </p>
          <button
            className="button primary"
            type="button"
            onClick={() => onReopen(request.question)}
          >
            원래 질문 다시 열기
          </button>
          {isRequester ? (
            <label className="checkbox-row section-gap">
              <input
                type="checkbox"
                checked={confirmSolved}
                onChange={(event) => setConfirmSolved(event.target.checked)}
              />
              결과를 확인했고 원래 질문이 해결되었습니다.
            </label>
          ) : (
            <p className="muted">해결 확인은 요청자가 수행합니다.</p>
          )}
        </>
      )}
      <div className="action-row">
        {next.map((status) => (
          <button
            key={status}
            type="button"
            className={`button ${status === 'ready' ? 'primary' : 'secondary'}`}
            disabled={(status === 'ready' && !linked) || (status === 'resolved' && !confirmSolved)}
            onClick={() => void transition(status)}
          >
            {requestStatusLabels[status]} 상태로 변경
          </button>
        ))}
      </div>
    </article>
  );
}
export default function RayportRequestsPage(
  props: WorkspacePageProps & { onReopen: (question: string) => void },
) {
  const [question, setQuestion] = useState('');
  const [purpose, setPurpose] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  return (
    <>
      <div className="page-heading">
        <div>
          <h1>생성 요청</h1>
          <p className="subtitle">필요한 기능을 요청하고 원래 질문의 해결까지 확인합니다.</p>
        </div>
        <button className="button primary" onClick={() => setShowCreate(!showCreate)}>
          {showCreate ? '닫기' : '새 요청'}
        </button>
      </div>
      {showCreate && (
        <form
          className="panel"
          onSubmit={(event) => {
            event.preventDefault();
            void props
              .run(() => props.api.createRequest(question, purpose), '생성 요청을 접수했습니다.')
              .then((result) => {
                if (result) {
                  setShowCreate(false);
                  setQuestion('');
                  setPurpose('');
                }
              });
          }}
        >
          <Field label="원래 질문">
            <textarea
              required
              rows={3}
              value={question}
              maxLength={2000}
              onChange={(event) => setQuestion(event.target.value)}
            />
          </Field>
          <Field label="업무 목적과 필요한 항목">
            <textarea
              required
              rows={3}
              value={purpose}
              maxLength={2000}
              onChange={(event) => setPurpose(event.target.value)}
            />
          </Field>
          <button className="button primary">요청 접수</button>
        </form>
      )}
      {props.workspace.requests.length === 0 ? (
        <section className="panel">
          <EmptyState title="접수된 요청이 없습니다">
            지원되지 않는 질문에서 바로 생성 요청을 만들 수 있습니다.
          </EmptyState>
        </section>
      ) : (
        props.workspace.requests.map((request) => (
          <RequestCard
            key={`${request.id}/${request.status}/${request.linked_tool_id}`}
            {...props}
            request={request}
          />
        ))
      )}
    </>
  );
}
