import { useState, type FormEvent } from 'react';
import {
  canManage,
  splitIdentifiers,
  toolStatusLabels,
} from '../../application/AppsWebSrcApplicationRayportSession';
import type { Tool } from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  downloadJson,
  EmptyState,
  Field,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

function ToolCard({
  tool,
  admin,
  api,
  run,
  onEdit,
}: Pick<WorkspacePageProps, 'api' | 'run'> & {
  tool: Tool;
  admin: boolean;
  onEdit: (tool: Tool) => void;
}) {
  const [evidence, setEvidence] = useState('');
  async function exportContract() {
    const contract = await run(() => api.toolContract(tool.id), '', false);
    if (contract) downloadJson(contract, `${tool.id}-v${contract.version}-contract.json`);
  }
  return (
    <article className="panel section-gap">
      <div className="section-heading">
        <h2>{tool.name}</h2>
        <span className={`pill ${tool.status === 'published' ? '' : 'neutral'}`}>
          {toolStatusLabels[tool.status]}
        </span>
      </div>
      <p>{tool.description || '설명 없음'}</p>
      <div className="result-meta">
        <span>
          도구{' '}
          <strong>
            {tool.id} · v{tool.version}
          </strong>
        </span>
        <span>
          담당 <strong>{tool.owner}</strong>
        </span>
        <span>
          데이터 경로{' '}
          <strong>
            {tool.source} / {tool.source_ref}
          </strong>
        </span>
      </div>
      <dl className="compact-details">
        <div>
          <dt>지원 범위</dt>
          <dd>
            영업일·점포 매출 / 최대 {tool.max_days}일 · {tool.max_rows}행
          </dd>
        </div>
        <div>
          <dt>허용 지연</dt>
          <dd>{tool.freshness_seconds.toLocaleString()}초</dd>
        </div>
        <div>
          <dt>검색 키워드</dt>
          <dd>{tool.keywords.join(', ') || '없음'}</dd>
        </div>
      </dl>
      {admin && tool.sql_template && (
        <details>
          <summary>SQL 템플릿</summary>
          <pre className="code-line">{tool.sql_template}</pre>
        </details>
      )}
      {tool.validation_evidence && <p className="callout">검증 근거: {tool.validation_evidence}</p>}
      {tool.status === 'published' && (
        <div className="action-row">
          <button className="button secondary" onClick={() => void exportContract()}>
            OpenAPI·도구 정의 내려받기
          </button>
          <span className="muted">MCP 도구 정의 JSON이며 MCP 서버 접속 기능은 아닙니다.</span>
        </div>
      )}
      {admin && tool.status === 'draft' && (
        <div className="section-gap">
          <Field
            label={`${tool.name} 검증 근거`}
            hint="업무 정의·권한·실행 범위와 대사 결과를 확인한 근거를 기록하세요."
          >
            <textarea
              rows={3}
              value={evidence}
              maxLength={4000}
              onChange={(event) => setEvidence(event.target.value)}
            />
          </Field>
          <button
            className="button secondary"
            disabled={!evidence.trim()}
            onClick={() =>
              void run(
                () => api.validateTool(tool.id, evidence.trim()),
                '도구 검증이 완료되었습니다.',
              )
            }
          >
            검증 기록 제출
          </button>
        </div>
      )}
      {admin && (
        <div className="action-row">
          {['draft', 'validated'].includes(tool.status) && (
            <button className="button secondary" onClick={() => onEdit(tool)}>
              초안 수정
            </button>
          )}
          {tool.status === 'validated' && (
            <button
              className="button primary"
              onClick={() => void run(() => api.publishTool(tool.id), '업무 도구를 게시했습니다.')}
            >
              검증된 버전 게시
            </button>
          )}
          {['draft', 'validated', 'published'].includes(tool.status) && (
            <button
              className="button secondary danger-text"
              onClick={() => void run(() => api.retireTool(tool.id), '도구 사용을 중지했습니다.')}
            >
              사용 중지
            </button>
          )}
        </div>
      )}
    </article>
  );
}
export default function RayportCatalogPage({ api, workspace, principal, run }: WorkspacePageProps) {
  const admin = canManage(principal);
  const [showCreate, setShowCreate] = useState(false);
  const blank: Tool = {
    id: '',
    name: '',
    description: '',
    keywords: [],
    source: 'mart',
    source_ref: '',
    sql_template: null,
    status: 'draft',
    version: 1,
    max_days: 31,
    max_rows: 1000,
    freshness_seconds: 86400,
    owner: principal.id,
    validation_evidence: null,
  };
  const [draft, setDraft] = useState<Tool>(blank);
  const [keywords, setKeywords] = useState('');
  const [editId, setEditId] = useState<string | null>(null);
  function change<K extends keyof Tool>(key: K, value: Tool[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }
  function edit(tool: Tool) {
    setDraft({ ...tool });
    setKeywords(tool.keywords.join(', '));
    setEditId(tool.id);
    setShowCreate(true);
  }
  async function create(event: FormEvent) {
    event.preventDefault();
    const input = { ...draft, keywords: splitIdentifiers(keywords) };
    const result = await run(
      () => (editId ? api.updateTool(editId, input) : api.createTool(input)),
      editId
        ? '초안을 수정했습니다. 새 버전을 다시 검증한 뒤 게시하세요.'
        : '업무 도구 초안을 저장했습니다.',
    );
    if (result) {
      setDraft(blank);
      setKeywords('');
      setEditId(null);
      setShowCreate(false);
    }
  }
  return (
    <>
      <div className="page-heading">
        <div>
          <h1>업무 API</h1>
          <p className="subtitle">업무 의미와 조회 범위를 확인하고 검증된 도구를 게시합니다.</p>
        </div>
        {admin && (
          <button
            className="button primary"
            onClick={() => {
              setShowCreate(!showCreate);
              setEditId(null);
              setDraft(blank);
              setKeywords('');
            }}
          >
            {showCreate ? '등록 닫기' : '도구 등록'}
          </button>
        )}
      </div>
      {admin && showCreate && (
        <form className="panel" onSubmit={(event) => void create(event)}>
          <h2>{editId ? `초안 수정 · 현재 v${draft.version}` : '새 도구 초안'}</h2>
          {editId && (
            <p className="callout">
              수정하면 검증 기록이 해제되고 새 초안 버전이 됩니다. 동시에 변경된 버전은 덮어쓰지
              않습니다.
            </p>
          )}
          <div className="form-grid">
            <Field label="도구 ID">
              <input
                required
                readOnly={Boolean(editId)}
                value={draft.id}
                maxLength={100}
                pattern="[A-Za-z0-9_.-]+"
                onChange={(event) => change('id', event.target.value)}
                placeholder="sales.store_daily"
              />
            </Field>
            <Field label="업무 이름">
              <input
                autoFocus
                required
                value={draft.name}
                maxLength={200}
                onChange={(event) => change('name', event.target.value)}
                placeholder="점포별 매출"
              />
            </Field>
          </div>
          <Field label="업무 정의">
            <textarea
              required
              rows={3}
              maxLength={4000}
              value={draft.description}
              onChange={(event) => change('description', event.target.value)}
              placeholder="순매출·취소·마감 기준과 지원 범위를 설명하세요."
            />
          </Field>
          <Field label="검색 키워드 (쉼표 구분)">
            <input
              required
              value={keywords}
              maxLength={1000}
              onChange={(event) => setKeywords(event.target.value)}
              placeholder="매출, 점포, 실적"
            />
          </Field>
          <div className="form-grid">
            <Field label="데이터 경로">
              <select
                value={draft.source}
                onChange={(event) => change('source', event.target.value as Tool['source'])}
              >
                <option value="mart">데이터 마트</option>
                <option value="apic">APIC 승인 조회</option>
              </select>
            </Field>
            <Field label="원천 참조 ID">
              <input
                required
                value={draft.source_ref}
                maxLength={100}
                onChange={(event) => change('source_ref', event.target.value)}
                list="mart-identifiers"
                placeholder="sample-sales"
              />
              <datalist id="mart-identifiers">
                {workspace.marts.map((mart) => (
                  <option value={mart.id} key={mart.id}>
                    {mart.name}
                  </option>
                ))}
              </datalist>
            </Field>
          </div>
          {draft.source === 'apic' && (
            <Field
              label="승인 대상 SQL 템플릿"
              hint="실행 경로는 서버의 APIC 연결 설정을 사용합니다. 입력만으로 운영 원천 실행이 활성화되지 않습니다."
            >
              <textarea
                required
                rows={5}
                spellCheck={false}
                value={draft.sql_template ?? ''}
                onChange={(event) => change('sql_template', event.target.value || null)}
                maxLength={20000}
              />
            </Field>
          )}
          <div className="form-grid">
            <Field label="최대 조회 일수">
              <input
                type="number"
                required
                min={1}
                max={366}
                value={draft.max_days}
                onChange={(event) => change('max_days', Number(event.target.value))}
              />
            </Field>
            <Field label="최대 반환 행">
              <input
                type="number"
                required
                min={1}
                max={10000}
                value={draft.max_rows}
                onChange={(event) => change('max_rows', Number(event.target.value))}
              />
            </Field>
            <Field label="허용 지연 (초)">
              <input
                type="number"
                required
                min={1}
                max={31536000}
                value={draft.freshness_seconds}
                onChange={(event) => change('freshness_seconds', Number(event.target.value))}
              />
            </Field>
            <Field label="업무 담당자">
              <input
                required
                value={draft.owner}
                maxLength={200}
                onChange={(event) => change('owner', event.target.value)}
              />
            </Field>
          </div>
          <button className="button primary">초안 저장</button>
        </form>
      )}
      {workspace.tools.length === 0 ? (
        <section className="panel">
          <EmptyState title="등록된 도구가 없습니다">
            관리자는 데이터 마트를 반입하거나 승인 SQL을 확인한 뒤 업무 도구를 등록할 수 있습니다.
          </EmptyState>
        </section>
      ) : (
        workspace.tools.map((tool) => (
          <ToolCard
            key={`${tool.id}/${tool.version}/${tool.status}`}
            tool={tool}
            admin={admin}
            api={api}
            run={run}
            onEdit={edit}
          />
        ))
      )}
    </>
  );
}
