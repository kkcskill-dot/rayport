import { useState } from 'react';
import { discoveryExample } from '../../application/AppsWebSrcApplicationRayportExamples';
import { formatTimestamp } from '../../application/AppsWebSrcApplicationRayportSession';
import type {
  Candidate,
  DiscoveryImport,
  SourceKind,
} from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  EmptyState,
  Field,
  JsonImport,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

function CandidateCard({
  candidate,
  api,
  run,
  principal,
  workspace,
}: WorkspacePageProps & { candidate: Candidate }) {
  const [name, setName] = useState(candidate.name);
  const [source, setSource] = useState<SourceKind>('mart');
  const [reference, setReference] = useState('');
  const [owner, setOwner] = useState(principal.id);
  return (
    <article className="panel section-gap">
      <div className="section-heading">
        <h2>{candidate.name}</h2>
        <span className="pill neutral">{candidate.converted_tool_id ? '초안 연결됨' : '후보'}</span>
      </div>
      <p className="muted">
        관측 자료에서 찾은 후보입니다. SQL만으로 업무 의미를 확정하지 않습니다.
      </p>
      <div className="details-grid">
        <div>
          <h3>관측 실행 수</h3>
          <p>{candidate.executions === null ? '미관측' : candidate.executions.toLocaleString()}</p>
        </div>
        <div>
          <h3>ASH 활성 표본</h3>
          <p>
            {candidate.ash_samples === null ? '미관측' : candidate.ash_samples.toLocaleString()}
          </p>
        </div>
        <div>
          <h3>모듈</h3>
          <p>{candidate.module || '미확인'}</p>
        </div>
      </div>
      <details>
        <summary>SQL과 식별 근거</summary>
        <pre className="code-line">{candidate.sql_template}</pre>
        <p className="muted wrap-anywhere">정규화 식별값: {candidate.signature}</p>
      </details>
      <div className="result-meta">
        <span>
          자료 출처 <strong>{candidate.source_label || '미기록'}</strong>
        </span>
        <span>
          관측 버전 <strong>{candidate.observation_version ?? '미기록'}</strong>
        </span>
        <span>
          반입 시각{' '}
          <strong>
            {candidate.observed_at ? formatTimestamp(candidate.observed_at) : '미기록'}
          </strong>
        </span>
        <span>
          관측 구간{' '}
          <strong>
            {candidate.window_start && candidate.window_end
              ? `${candidate.window_start} ~ ${candidate.window_end}`
              : '미확인 · 구간별 빈도 비교 불가'}
          </strong>
        </span>
      </div>
      {candidate.converted_tool_id ? (
        <p className="callout">
          연결된 도구: {candidate.converted_tool_id}. 업무 API에서 정의·검증·게시를 진행하세요.
        </p>
      ) : (
        <form
          className="section-gap"
          onSubmit={(event) => {
            event.preventDefault();
            void run(
              () =>
                api.draftCandidate(candidate.id, { name, source, source_ref: reference, owner }),
              '후보에서 업무 API 초안을 만들었습니다.',
            );
          }}
        >
          <div className="form-grid">
            <Field label="확인할 업무 이름">
              <input
                required
                maxLength={200}
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            <Field label="담당자">
              <input
                required
                maxLength={200}
                value={owner}
                onChange={(event) => setOwner(event.target.value)}
              />
            </Field>
            <Field label="제공 경로">
              <select
                value={source}
                onChange={(event) => setSource(event.target.value as SourceKind)}
              >
                <option value="mart">마트</option>
                <option value="apic">APIC 승인 조회</option>
              </select>
            </Field>
            <Field label="원천 참조 ID">
              <input
                required
                value={reference}
                maxLength={100}
                list={`mart-${candidate.id}`}
                onChange={(event) => setReference(event.target.value)}
              />
              <datalist id={`mart-${candidate.id}`}>
                {workspace.marts.map((mart) => (
                  <option key={mart.id} value={mart.id}>
                    {mart.name}
                  </option>
                ))}
              </datalist>
            </Field>
          </div>
          <button className="button primary">API 초안 만들기</button>
        </form>
      )}
    </article>
  );
}
export default function RayportCandidatesPage(props: WorkspacePageProps) {
  const [authorized, setAuthorized] = useState(false);
  return (
    <>
      <h1>업무 API 발굴</h1>
      <p className="subtitle">승인된 SQL 관측 자료를 반입하고 업무 API 후보로 연결합니다.</p>
      <div className="callout">
        ASH는 실행 횟수가 아닌 활성 세션 표본입니다. 실행 수가 누락되면 0으로 간주하지 않습니다.
        원본 AWR HTML을 직접 분석하는 기능은 현재 제공하지 않습니다.
      </div>
      <label className="checkbox-row section-gap">
        <input
          type="checkbox"
          checked={authorized}
          onChange={(event) => setAuthorized(event.target.checked)}
        />
        이 관측 자료의 사용 권한과 필요한 Diagnostics Pack 사용권을 확인했습니다.
      </label>
      <JsonImport
        label="SQL 관측 JSON 반입"
        description="민감한 리터럴·바인드값을 제거한 관측 자료를 사용하세요. 최대 2MB."
        example={discoveryExample()}
        buttonLabel="확인한 자료에서 후보 생성"
        onImport={async (value) => {
          if (!authorized) throw new Error('자료와 Diagnostics 사용 권한을 먼저 확인하세요.');
          const input = value as DiscoveryImport;
          if (!Array.isArray(input.observations) || !input.source_label)
            throw new Error('source_label과 observations 배열이 필요합니다.');
          return Boolean(
            await props.run(
              () => props.api.importDiscovery({ ...input, diagnostics_authorized: true }),
              '관측 자료에서 후보를 생성했습니다.',
            ),
          );
        }}
      />
      {props.workspace.candidates.length === 0 ? (
        <section className="panel section-gap">
          <EmptyState title="아직 발굴한 후보가 없습니다">
            JSON 반입 후 관측 근거와 SQL을 확인할 수 있습니다.
          </EmptyState>
        </section>
      ) : (
        props.workspace.candidates.map((candidate) => (
          <CandidateCard key={candidate.id} {...props} candidate={candidate} />
        ))
      )}
    </>
  );
}
