import { useState } from 'react';
import { martExample } from '../../application/AppsWebSrcApplicationRayportExamples';
import type { MartImport } from '../../domain/AppsWebSrcDomainRayportTypes';
import {
  EmptyState,
  JsonImport,
  type WorkspacePageProps,
} from '../components/AppsWebSrcPresentationComponentsRayportControls';

export default function RayportMartsPage({ api, workspace, run }: WorkspacePageProps) {
  const [confirmReplace, setConfirmReplace] = useState(false);
  return (
    <>
      <h1>데이터 마트</h1>
      <p className="subtitle">
        반복 조회에 필요한 영업일·점포별 실적을 완성된 스냅샷으로 관리합니다.
      </p>
      <div className="callout">
        1.0은 JSON으로 반입한 전체 스냅샷을 검증해 새 버전으로 게시합니다. 원천 자동 적재·CDC·정기
        갱신은 연결되지 않았습니다. 삭제·정정을 반영할 때도 미변경 데이터를 포함한 전체 스냅샷을
        반입하세요.
      </div>
      <div className="section-gap">
        {workspace.marts.length === 0 ? (
          <section className="panel">
            <EmptyState title="등록된 마트가 없습니다">
              먼저 데이터를 반입한 뒤 업무 API의 원천 참조 ID로 연결하세요.
            </EmptyState>
          </section>
        ) : (
          workspace.marts.map((mart) => (
            <article className="panel section-gap" key={`${mart.id}/${mart.version}`}>
              <div className="section-heading">
                <h2>{mart.name}</h2>
                <span className="pill">버전 {mart.version}</span>
              </div>
              <div className="details-grid">
                <div>
                  <h3>참조 ID</h3>
                  <p>{mart.id}</p>
                </div>
                <div>
                  <h3>전체 행 수</h3>
                  <p>{mart.rows_count.toLocaleString()}</p>
                </div>
                <div>
                  <h3>완료 영업일</h3>
                  <p>{mart.complete_through}</p>
                </div>
              </div>
              <p className="muted">데이터 기준 시각: {mart.data_as_of}</p>
            </article>
          ))
        )}
      </div>
      <label className="checkbox-row section-gap">
        <input
          type="checkbox"
          checked={confirmReplace}
          onChange={(event) => setConfirmReplace(event.target.checked)}
        />
        같은 ID가 있으면 전체 데이터를 새 버전으로 교체합니다. 미변경 기간·점포도 포함했음을
        확인했습니다.
      </label>
      <JsonImport
        label="마트 스냅샷 반입"
        description="영업일·점포·통화별 한 행을 갖는 전체 JSON 스냅샷을 반입합니다. 금액은 KRW 원 정수입니다."
        example={martExample()}
        buttonLabel="검증 후 새 버전 게시"
        onImport={async (value) => {
          const input = value as MartImport;
          if (!input.id || !Array.isArray(input.rows))
            throw new Error('마트 id와 rows 배열이 필요합니다.');
          if (workspace.marts.some((mart) => mart.id === input.id) && !confirmReplace)
            throw new Error('기존 마트의 전체 스냅샷 교체를 확인하세요.');
          const result = await run(
            () => api.importMart(input),
            '마트 스냅샷을 검증하고 게시했습니다.',
          );
          if (result) setConfirmReplace(false);
          return Boolean(result);
        }}
      />
    </>
  );
}
