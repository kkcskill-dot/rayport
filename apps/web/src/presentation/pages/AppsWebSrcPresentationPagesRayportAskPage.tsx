import { useState, type FormEvent } from 'react';
import {
  formatWon,
  splitIdentifiers,
  validateQuery,
} from '../../application/AppsWebSrcApplicationRayportSession';
import type { QueryResult, Resolution } from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  EmptyState,
  ErrorText,
  Field,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

export default function RayportAskPage({
  api,
  workspace,
  principal,
  run,
  initialQuestion,
  onRequestCreated,
}: WorkspacePageProps & { initialQuestion: string; onRequestCreated: () => void }) {
  const [question, setQuestion] = useState(initialQuestion);
  const [purpose, setPurpose] = useState('');
  const [resolution, setResolution] = useState<Resolution | null>(null);
  const [selectedId, setSelectedId] = useState('');
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');
  const [stores, setStores] = useState('');
  const [result, setResult] = useState<QueryResult | null>(null);
  const [error, setError] = useState('');
  const published = workspace.tools.filter((tool) => tool.status === 'published');
  const tool = published.find((item) => item.id === selectedId);
  async function resolve(event: FormEvent) {
    event.preventDefault();
    setResolution(null);
    setResult(null);
    setSelectedId('');
    setError('');
    const answer = await run(() => api.resolve(question.trim()), '', false);
    if (answer) {
      setResolution(answer);
      if (answer.status === 'matched' && answer.tool_id) setSelectedId(answer.tool_id);
    }
  }
  async function query(event: FormEvent) {
    event.preventDefault();
    if (!tool) return;
    setResult(null);
    const input = {
      start_date: start,
      end_date_exclusive: end,
      store_ids: splitIdentifiers(stores),
    };
    const problem = validateQuery(input, tool);
    setError(problem ?? '');
    if (problem) return;
    const answer = await run(() => api.query(tool.id, input), '', false);
    if (answer) setResult(answer);
  }
  async function createRequest() {
    const request = await run(
      () => api.createRequest(question.trim(), purpose.trim()),
      '기능 생성 요청을 접수했습니다.',
    );
    if (request) onRequestCreated();
  }
  function clearResult() {
    setResult(null);
    setError('');
  }
  return (
    <>
      <h1>업무 데이터 질문</h1>
      <p className="subtitle">게시된 업무 도구를 찾아 조건에 맞는 데이터를 조회합니다.</p>
      <form className="ask-box" onSubmit={(event) => void resolve(event)}>
        <label htmlFor="question">어떤 데이터가 필요한가요?</label>
        <div className="question-input">
          <input
            id="question"
            value={question}
            onChange={(event) => {
              setQuestion(event.target.value);
              setResolution(null);
              setSelectedId('');
              clearResult();
            }}
            placeholder="예: 점포별 매출을 확인하고 싶어요"
            maxLength={2000}
            required
          />
          <button className="button primary">도구 찾기</button>
        </div>
        <p className="muted section-gap">
          현재는 등록된 이름·키워드로 도구를 찾습니다. AI 모델을 통한 자유 분석은 연결되지
          않았습니다.
        </p>
      </form>
      {resolution && (
        <section className="panel section-gap">
          <h2>
            {resolution.status === 'matched'
              ? '사용할 수 있는 도구를 찾았습니다'
              : resolution.status === 'ambiguous'
                ? '업무 도구를 선택해 주세요'
                : '추가 기능이 필요합니다'}
          </h2>
          <p>{resolution.explanation}</p>
          {resolution.suggestions.length > 0 && (
            <p className="muted">관련 도구: {resolution.suggestions.join(', ')}</p>
          )}
          {resolution.status === 'missing' && (
            <>
              <Field label="요청 목적과 필요한 항목">
                <textarea
                  value={purpose}
                  onChange={(event) => setPurpose(event.target.value)}
                  placeholder="필요한 지표·차원·업무 목적을 적어 주세요."
                  maxLength={2000}
                  rows={3}
                />
              </Field>
              <button
                className="button primary"
                type="button"
                disabled={!question.trim() || !purpose.trim()}
                onClick={() => void createRequest()}
              >
                생성 요청 접수
              </button>
            </>
          )}
        </section>
      )}
      <section className="panel section-gap">
        <h2>조회 조건</h2>
        {published.length === 0 ? (
          <EmptyState title="게시된 업무 도구가 없습니다">
            업무 API 등록과 검증을 완료하거나 필요한 기능을 생성 요청하세요.
          </EmptyState>
        ) : (
          <form onSubmit={(event) => void query(event)}>
            <Field label="업무 도구">
              <select
                value={selectedId}
                required
                onChange={(event) => {
                  setSelectedId(event.target.value);
                  clearResult();
                }}
              >
                <option value="">도구를 선택하세요</option>
                {published.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name} · v{item.version}
                  </option>
                ))}
              </select>
            </Field>
            {tool && (
              <>
                <p>{tool.description}</p>
                <p className="muted">
                  한 번에 최대 {tool.max_days}일 · 최대 {tool.max_rows.toLocaleString()}행 · 허용
                  지연 {tool.freshness_seconds.toLocaleString()}초
                </p>
                <div className="form-grid">
                  <Field label="시작일">
                    <input
                      type="date"
                      required
                      value={start}
                      onChange={(event) => {
                        setStart(event.target.value);
                        clearResult();
                      }}
                    />
                  </Field>
                  <Field label="종료일 (당일 미포함)">
                    <input
                      type="date"
                      required
                      value={end}
                      onChange={(event) => {
                        setEnd(event.target.value);
                        clearResult();
                      }}
                    />
                  </Field>
                </div>
                <Field
                  label="점포 ID"
                  hint={`${tool.source === 'apic' ? 'APIC 조회는 점포 한 개를 지정하세요.' : '쉼표로 구분하며 비우면 허용된 전체 점포를 조회합니다.'} 내 점포: ${principal.store_ids.join(', ') || '없음'}`}
                >
                  <input
                    value={stores}
                    onChange={(event) => {
                      setStores(event.target.value);
                      clearResult();
                    }}
                    placeholder="S01, S02"
                    maxLength={2000}
                  />
                </Field>
                <ErrorText>{error}</ErrorText>
                <button className="button primary">권한과 범위를 확인하고 조회</button>
              </>
            )}
          </form>
        )}
      </section>
      {result && (
        <section className="panel section-gap" aria-label="조회 결과">
          <div className="section-heading">
            <h2>조회 결과</h2>
            <span className="pill">{result.rows.length.toLocaleString()}행</span>
          </div>
          <div className="result-meta">
            <span>
              기준 시각 <strong>{result.data_as_of}</strong>
            </span>
            <span>
              데이터 버전 <strong>{result.dataset_version}</strong>
            </span>
            <span>
              조회 경로 <strong>{result.source}</strong>
            </span>
            <span>
              기간{' '}
              <strong>
                {start} ≤ 영업일 &lt; {end}
              </strong>
            </span>
          </div>
          {result.rows.length === 0 ? (
            <EmptyState title="해당 조건의 데이터가 없습니다">
              데이터 미확보와 실제 실적 0은 다를 수 있습니다. 마트 확보 범위를 확인하세요.
            </EmptyState>
          ) : (
            <>
              <div className="summary-grid">
                <div className="metric">
                  <span>총매출</span>
                  <strong>{formatWon(result.gross_minor)}</strong>
                </div>
                <div className="metric">
                  <span>취소 금액</span>
                  <strong>{formatWon(result.cancel_minor)}</strong>
                </div>
                <div className="metric">
                  <span>순매출</span>
                  <strong>{formatWon(result.net_minor)}</strong>
                </div>
              </div>
              <div className="table-scroll">
                <table>
                  <caption className="sr-only">기간과 점포별 매출 결과. 금액 단위 원.</caption>
                  <thead>
                    <tr>
                      <th>영업일</th>
                      <th>점포</th>
                      <th>총매출</th>
                      <th>취소</th>
                      <th>순매출</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.rows.map((row) => (
                      <tr key={`${row.business_date}/${row.store_id}/${row.currency}`}>
                        <td>{row.business_date}</td>
                        <td>{row.store_id}</td>
                        <td>{formatWon(row.gross_minor)}</td>
                        <td>{formatWon(row.cancel_minor)}</td>
                        <td>{formatWon(row.gross_minor - row.cancel_minor)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <details className="section-gap">
                <summary>순매출 흐름 보기 (처음 12행)</summary>
                <div className="data-bars" role="img" aria-label="날짜와 점포별 순매출 비교">
                  {result.rows.slice(0, 12).map((row) => {
                    const net = row.gross_minor - row.cancel_minor;
                    const max = Math.max(
                      1,
                      ...result.rows
                        .slice(0, 12)
                        .map((value) => Math.abs(value.gross_minor - value.cancel_minor)),
                    );
                    return (
                      <div className="data-bar-row" key={`${row.business_date}/${row.store_id}`}>
                        <span>
                          {row.business_date} · {row.store_id}
                        </span>
                        <div>
                          <i
                            style={{
                              width: `${(Math.abs(net) / max) * 100}%`,
                              background: net < 0 ? '#b24a3b' : 'var(--primary)',
                            }}
                          />
                        </div>
                        <strong>{formatWon(net)}</strong>
                      </div>
                    );
                  })}
                </div>
              </details>
            </>
          )}
        </section>
      )}
    </>
  );
}
