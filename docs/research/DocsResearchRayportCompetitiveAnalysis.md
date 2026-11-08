# 레이포트 유사 솔루션 비교와 차별화 근거

확인일: 2026-11-08. 공식 제품·기술 문서 기준. **경쟁 제품의 현재 기능과 레이포트의 제안 설계를 구분한다. 레이포트의 성능·비용 우위는 아직 측정하지 않았다.**

## 1. 제품 포지션

레이포트는 **기존 업무 SQL을 검증된 조회 서비스로 전환하고, 반복 수요를 마트로 흡수하는 서버 설치형 업무 데이터 플랫폼**으로 설계한다. 자연어 질문은 진입점이고, 관리 단위는 업무 의미·권한·신선도·실행 예산이 붙은 조회 서비스다.

기존 사용자 결정에 따라 APIC의 원천 실행을 재사용하고, MCP는 현재 구현을 확인한 뒤 재사용하거나 어댑터로 제공한다. 레이포트는 업무 의미·질문·생성 요청·부하 통제·마트 운영에 집중한다. MSTR/Strategy 보고서군은 결과와 권한 동등성을 검증한 뒤 점진적으로 옮긴다.

다음 기능은 이미 시장에 존재하므로 단독 차별점으로 주장하면 안 된다.

- 자연어 분석, 의미 계층, 검증된 질의: Strategy, Snowflake, Cube, ThoughtSpot 등.
- SQL/API 자동화: Hasura Native Queries·RESTified Endpoints, PostgREST, Denodo 등.
- 캐시와 사전 집계: Strategy Intelligent Cubes, Cube pre-aggregations, Dremio Reflections, Denodo 등.
- 사용 이력에 따른 최적화: Dremio Autonomous Reflections.
- 기존 질의에 맞지 않는 사용자 질문 수집과 개선 후보 제안: Snowflake Cortex Analyst.

따라서 차별화의 중심은 **Oracle을 포함한 기존 운영계의 업무 흔적 → 업무 의미 확인 → APIC 실행 계약 → 부하 예산과 신선도 검증 → API/마트 배포 → 미충족 질문을 담당자의 생성 요청으로 연결**하는 운영 흐름이다. 이 조합도 독점 기능으로 단정하지 않고 실제 도입 시간과 운영 부담으로 입증한다.

## 2. 비교표

| 제품군 | 공식 문서로 확인한 강점과 겹치는 기능 | 서버 설치·운영 조건 | 레이포트가 검증해야 할 차이 |
| --- | --- | --- | --- |
| **Strategy One / Mosaic (기존 MSTR)** | 업무 지표와 의미 계층, 보고서·대시보드·AI, Intelligent Cubes와 캐시. Mosaic은 의미 계층을 다른 도구까지 확장 | 설치형 기존 환경과 새 Mosaic/AI 제공 범위를 같은 것으로 가정하면 안 됨. 도입 버전·계약·지원 범위별 확인 | 기존 보고서의 지표·프롬프트·권한·소계·배포 요건을 보존하면서 APIC 조회 서비스로 전환하는 시간과 비용. [의미 계층](https://software.strategy.com/semantic-layer), [캐시·큐브](https://www2.strategy.com/producthelp/Current/workstation/en-us/Content/application_cache_intelligent_cube_settings.htm) |
| **Snowflake Cortex Analyst / MCP / Dynamic Tables** | 의미 모델·검증 질의로 자연어 SQL 생성, 관리형 MCP, SQL/질문 이력의 미등록 지표·필터·검증 질의 추천, 신선도 목표 기반 물리화 | Snowflake 플랫폼은 공용 클라우드 기반 관리 서비스이며 로컬·사설 클라우드에 설치할 수 없음. 원천 수집·복제는 별도 경로 설계 필요 | 폐쇄망 설치, 기존 Oracle/APIC 자산 재사용, 원천별 실행 예산. 자연어·MCP·질문 개선 후보 자체는 중복. [Analyst](https://docs.snowflake.com/en/user-guide/snowflake-cortex/cortex-analyst), [배포 구조](https://docs.snowflake.com/en/user-guide/intro-key-concepts) |
| **Denodo** | 가상화·연합 조회, 의미 계층·데이터 서비스, SQL/REST/GraphQL, 선택적 캐시·집계·복제, AI 지원 | 서버/컨테이너 배포 및 관리형 서비스 선택. 구독 티어마다 규모·기능 조건 확인 | 통합 엔진 전반과 경쟁하기보다 기존 SQL의 API 후보화, 원천 보호 정책, 수요에서 배포까지의 업무 경험을 좁게 최적화. [플랫폼](https://www.denodo.com/en/denodo-platform/denodo-platform), [구독](https://www.denodo.com/en/denodo-platform/subscriptions), [Docker](https://www.denodo.com/en/page/denodo-docker-faq) |
| **Dremio** | 연합 SQL·레이크하우스·의미 계층, Reflections로 원본 스캔 회피, 실제 질의 패턴을 바탕으로 자동 물리화 | 공식 에디션 표상 Community/Enterprise는 자체 운영 가능. Autonomous Reflections는 Enterprise에 제공되며 기본 off, Community 미제공. Cloud에도 제공 | Oracle 업무 SQL과 업무 담당자 검증을 출발점으로 하는 API 제품화. 자동 마트나 원천 접근 감소 자체는 차별점 아님. [가속](https://docs.dremio.com/current/acceleration/), [에디션](https://docs.dremio.com/editions/) |
| **Cube** | 공통 지표·차원·접근 규칙, 여러 소비 인터페이스, pre-aggregation, rollup-only 실행으로 사용자 질의의 원천 접근 차단 | Cube Core 자체 설치 가능. 운영 예제는 API·refresh worker·Cube Store router/worker로 구성. 상용 플랫폼의 AI·운영 기능을 Core와 동일시하지 않음 | 의미 계층을 처음부터 모델링하는 노력과 운영 SQL에서 검증된 정의를 이끌어내는 노력 비교. 설치·운영 프로세스 수 역시 실제 측정. [Core 배포](https://docs.cube.dev/admin/deployment/core), [사전 집계](https://docs.cube.dev/docs/pre-aggregations/using-pre-aggregations) |
| **Hasura / PostgREST** | Hasura는 DB 객체·raw SQL에서 GraphQL 생성, REST 엔드포인트와 OpenAPI 제공. PostgREST는 PostgreSQL 스키마·권한에서 REST 제공 | Hasura v2 자체 설치와 DDN의 배포·제품 범위를 구분. DDN 공식 FAQ는 엔진·커넥터 자체 운영 가능이라 설명. PostgREST는 독립 실행 서버 | API 자동 생성 자체보다 ASH/AWR의 관찰 근거, 업무 의미, 마트·부하 계약, 생성 요청의 결합. 원천 실행은 APIC 자산을 우선 활용. [Native Queries](https://hasura.io/docs/2.0/schema/postgres/native-queries/), [REST](https://hasura.io/docs/2.0/restified/overview/), [DDN FAQ](https://hasura.io/docs/3.0/help/faq/), [PostgREST](https://docs.postgrest.org/en/stable/) |
| **Metabase / Apache Superset** | 셀프서비스 질의·차트·대시보드와 캐시. Metabase는 자연어 Metabot과 테이블을 생성하는 transforms까지 제공 | 자체 설치 가능. Metabase의 일부 캐시 정책·자동 갱신은 Pro/Enterprise, 고급 transforms는 별도 조건. Superset은 오픈소스이며 운영 인프라는 자체 관리 | BI 화면 재구현 범위를 줄이고 승인된 업무 API·기능 요청·원천 보호를 중심으로 구성. 단순 대시보드만 필요하면 기존 BI가 충분할 수 있음. [Metabot](https://www.metabase.com/docs/latest/ai/metabot), [transforms](https://www.metabase.com/docs/latest/data-modeling/transforms/transforms-overview), [Metabase 캐시](https://www.metabase.com/docs/latest/configuring-metabase/caching), [Superset](https://superset.apache.org/), [Superset 캐시](https://superset.apache.org/admin-docs/configuration/cache/) |
| **ThoughtSpot Spotter** | 관리되는 의미 계층 위에서 자연어 질문을 해석하고 확인 가능한 질의 근거 제공 | 확인한 현재 문서는 ThoughtSpot Cloud 중심. 과거 설치형 제공 이력만으로 현재 Spotter의 폐쇄망 설치 가능 여부를 단정하지 않음 | 사용자 경험의 비교 대상. 한국어 업무 용어·미지원 질문 처리·원천 보호 상태 설명·API 생성 요청 완료까지 검증. [Spotter](https://www.thoughtspot.com/product/agents/spotter), [현재 문서](https://docs.thoughtspot.com/) |

위 표는 기능 점수표가 아니다. 미확인 기능을 ‘없음’으로 표시하지 않았으며 실제 제품·에디션·버전에 따라 도입 결과가 달라진다.

## 3. 특히 혼동하지 말아야 할 중복 기능

### Snowflake: ‘질문에서 개선 후보로’도 이미 존재

Cortex Analyst는 SQL 이력에서 검증 질의·지표·필터를 추천하며, 기존 검증 질의에 대응되지 않는 빈번한 사용자 질문도 묶어 개선 후보로 제시한다. 후보는 사람이 수락·수정·기각한다. 따라서 레이포트의 기능 생성 요청은 단순 피드백 저장을 넘어 **담당자·기한·권한·검증 증거·API 버전·배포 결과까지 연결**해야 한다. 이 차이는 설계 제안이며 경쟁 제품 전체에 해당 운영 흐름이 없다는 뜻은 아니다. [공식 제안 기능](https://docs.snowflake.com/en/user-guide/views-semantic/verified-query-suggestions)

Snowflake 관리형 MCP는 Cortex Analyst/Search/Agents, 사용자 도구 및 SQL 실행을 노출할 수 있다. 레이포트의 MCP 제공 여부 자체보다 APIC의 기존 도구에 동일 권한·부하 예산을 일관되게 적용하는 것이 중요하다. [관리형 MCP](https://docs.snowflake.com/en/user-guide/snowflake-cortex/cortex-agents-mcp)

### Dremio·Cube: ‘반복 조회를 사전 구성’도 이미 존재

Dremio Autonomous Reflections는 실제 질의 패턴으로 물리화를 생성·관리한다. 확인한 Cloud 상세 문서는 Iceberg/Parquet 및 그 위의 뷰를 대상으로 명시한다. 이 제약을 모든 설치형 버전의 제약으로 일반화하지 않는다. [Cloud Autonomous Reflections](https://docs.dremio.com/dremio-cloud/admin/performance/autonomous-reflections/)

Cube는 집계 정의에 맞는 질의를 사전 집계로 처리하며 rollup-only 모드로 사용자 질의의 원천 실행을 차단할 수 있다. 레이포트 역시 ‘캐시 미스면 운영 DB 실행’을 기본으로 삼지 않아야 한다. 차이는 개별 기능보다 **원천 DB별 업무 시간·동시 실행·허용 지연을 함께 관리하는 경험**으로 검증한다. [Cube pre-aggregations](https://docs.cube.dev/docs/pre-aggregations/using-pre-aggregations)

### Strategy: ‘의미 계층 + 캐시’만으로 대체 설득 불가

Strategy는 이미 의미 계층, 캐시·Intelligent Cubes, 동적 소싱과 AI 기능을 갖는다. 레이포트의 대체 대상은 제품 전체가 아니라 업무 보고서군 단위로 잡는다. 정형 인쇄·복잡한 소계·다단 프롬프트·구독 배포 등이 필요한 보고서는 요구 충족 전까지 유지한다. [Strategy 의미 계층](https://software.strategy.com/semantic-layer), [큐브 설정](https://www2.strategy.com/producthelp/Current/workstation/en-us/Content/application_cache_intelligent_cube_settings.htm)

### API 자동화: SQL을 엔드포인트로 감싸는 것만으로 부족

Hasura Native Queries는 raw SQL을 GraphQL로 노출하며, RESTified Endpoints는 파라미터·권한과 OpenAPI를 제공한다. 레이포트는 ASH/AWR SQL을 그대로 발행하는 대신 **업무 의미와 사용자 권한, 결과 스키마, 허용 파라미터, 신선도, 실행 경로, 취소 가능성**을 완성해야 한다. [Native Queries](https://hasura.io/docs/2.0/schema/postgres/native-queries/), [RESTified Endpoints](https://hasura.io/docs/2.0/restified/overview/)

## 4. DW·DM과 공존하는 경계

DW·DM은 데이터 저장·통합·이력·정합성 문제를 담당하며, 레이포트는 업무 수요에 맞는 조회 서비스의 발견·검증·제공·운영을 담당한다. 최신 DW도 실시간 수집·의미 계층·AI를 지원하므로 ‘DW는 느린 배치만 가능’이라는 비교는 사용하지 않는다.

| 영역 | 주된 책임 | 레이포트 정책 제안 |
| --- | --- | --- |
| 기존 DW | 전사 공통 사실·차원·이력·정합성 | 동일한 확정 지표가 있으면 우선 재사용. 다른 이름의 중복 정의를 만들지 않음 |
| 기존 DM | 부서·주제별 조회 최적화 | 적합한 마트는 그대로 서비스 원천으로 등록하고 갱신 상태를 표시 |
| 레이포트 업무 마트 | 반복되는 승인 API/질문의 제공 비용 감소 | 소유자·계산식·차원·보관 기간·갱신 예산·폐기 기준을 갖는 최소 목적 마트 |
| APIC | 기존 원천 실행·도구/MCP 진입 | 원천 연결·실행 자산 재사용. Rayport 계약의 권한·제한을 우회하지 않는 실행 경로로 설계 |
| 레이포트 UI/API | 질문·발견·생성 요청·제공 품질 | 업무 의미와 신선도·권한을 만족하는 경로 선택, 불가하면 요청/대기/거절을 명확히 표시 |

**레이포트 업무 마트도 기술적으로 DM이다.** ‘DM이 아니다’라는 주장이 아니라, 사용 수요에 의해 작게 만들고 수명주기와 원천 비용을 제품에서 관리한다는 운영 방식으로 구분한다. 장기 전사 이력 저장이나 모든 원천의 전면 복제는 초기 범위로 잡지 않는다.

## 5. 설득력 있는 차별점과 입증 방법

| 차별화 가설 | 제품에서 보여줄 것 | 실증 기준 |
| --- | --- | --- |
| 기존 운영 업무를 빠르게 서비스화 | SQL 관찰 근거, 원래 화면/배치와의 연결, 업무 의미 확인, API 초안 | 동일 업무 API의 최초 초안 시간·승인까지의 총 소요 시간. 사람 수정량과 반려 사유까지 집계 |
| 운영 DB 영향에 대한 통제 | 원천별 예산, 현재 실행 수, 갱신 부하, 차단·취소 결과 | 동일 데이터 범위·신선도에서 원천 executions/CPU/논리 읽기/물리 읽기/운영 p95 지연 비교 |
| 반복 업무의 관리 가능한 사전 구성 | 마트 전환 근거, 예상 재사용 수, 갱신 비용, 만료·폐기 | 직접 조회 비용 대비 초기 구축+갱신+저장+제공 총비용. 업무 시간대별 변화 포함 |
| 질문의 실패가 요청 완료로 연결 | 질문→누락 능력→요청서→담당자→검증→배포→원래 질문 재실행 | 중복 요청 병합률, 생성 요청 완료율, 완료 시간, 재실행 성공률 |
| 기존 MSTR 보고서군의 점진 대체 | 보고서-지표-API 매핑, 결과·권한 대조, 되돌리기 | 대표 기간·사용자 권한·프롬프트 조합에서 결과 동등성. 대체 가능한 보고서 수와 유지 대상 분리 |
| 서버 설치형 운영의 단순성 | Rust 서비스+React 배포, 폐쇄망 의존성 명세, 백업·복구 | 설치 절차·상주 프로세스·유휴/부하 메모리·업그레이드·복구 시간의 실제 측정 |

‘부하 90% 감소’, ‘MSTR 완전 대체’, ‘SQL 자동 이해 정확도 99%’ 같은 수치는 현재 근거가 없다. 목표치를 별도로 정하더라도 측정값처럼 화면에 표시하지 않는다.

## 6. 비교 PoC 설계

1. **동일 업무 묶음 선정:** 기존 MSTR의 반복 집계, 조건부 상세 조회, 다단 프롬프트 보고서와 신규 질문을 포함한다. 업무 담당자가 정답 계산식·시간대·지연 허용치를 먼저 정의한다.
2. **동일 입력·권한으로 대조:** 기간·차원·조직 권한·개인정보 마스킹·취소/반품/마감 기준을 맞춘다. 단순 합계 일치만으로 동등성을 판정하지 않는다.
3. **동일 신선도로 비용 측정:** 즉시 조회와 하루 전 마트의 응답속도를 그대로 비교하지 않는다. 캐시 적중/미스·최초 적재·갱신·원천 취소·장애 상태를 모두 기록한다.
4. **운영 DB 측정은 사전 합의 범위에서:** 고객 DBA가 승인한 관찰·검증 창을 사용한다. ASH/AWR 수집 가능 범위와 라이선스·권한 조건은 연결별로 확인한다. 운영 DB를 대상으로 임의 부하 시험을 하지 않는다.
5. **사용성 검증:** 현업이 SQL을 보지 않고 지표 뜻·신선도·누락된 기능과 요청 상태를 이해할 수 있는지 평가한다. 설계 시안과 실제 동작 결과는 구분한다.

## 7. 우선순위 제안

- 1차 비교 기준 제품: **기존 Strategy 환경, Snowflake Cortex Analyst, Denodo**. 각각 실제 대체 대상, AI 의미 계층·요청 개선 경쟁, 가상화·API 경쟁에 해당한다.
- 마트·실행 정책 설계 참고: **Dremio와 Cube**. 자동 물리화·rollup-only·갱신 정책에서 배울 부분이 크다.
- API 계약 참고: **Hasura/PostgREST**. APIC와 겹치는 실행 기능을 새로 만드는 범위를 줄인다.
- 현업 사용성 참고: **Metabase와 ThoughtSpot**. 질문·결과 확인·수정 경험을 비교한다. Superset은 시각화 범위의 참고 대상으로 둔다.

최초 데모는 ‘질문에 대한 답변 → 근거와 신선도 확인 → 없는 기능 요청 → 기존 SQL 근거의 API 후보 검토 → 승인 → 반복 수요의 마트 전환’ 한 흐름을 끝까지 보여준다. 다양한 차트 수보다 이 흐름의 완결성과 원천 보호 증거가 초기 가치다.

## 8. 확인 범위와 미확인 사항

- 문서의 ‘확인’은 공개 공식 자료 조사이며, 경쟁 제품 설치·성능시험·구매 조건 검증을 수행한 것은 아니다.
- Denodo 티어별 상세 체크 표시, ThoughtSpot 현재 폐쇄망 지원, Strategy Mosaic/AI의 고객 설치형 제공 범위, Hasura DDN 상용 운영 조건은 실제 견적·버전 기준으로 추가 확인한다.
- Hasura v2 서버 설정에서 Redis 기반 query caching 등은 Enterprise 항목으로 표시된다. 이를 DDN 전체의 과금 조건으로 일반화하지 않는다. [v2 서버 설정](https://hasura.io/docs/2.0/deployment/graphql-engine-flags/reference/)
- Metabase 공식 문서는 Metabot을 모든 플랜에서 이용 가능하다고 설명한다. AI 연결·사용량·폐쇄망 작동 조건은 별도 확인 대상이다. transforms의 기본/고급 구분과 캐시 자동 갱신의 권한 제약도 적용 버전에서 확인한다. [Metabot](https://www.metabase.com/docs/latest/ai/metabot), [transforms](https://www.metabase.com/docs/latest/data-modeling/transforms/transforms-overview), [캐시](https://www.metabase.com/docs/latest/configuring-metabase/caching)
- Superset의 `Next` 문서와 실험 기능을 현재 고객 운영 버전의 안정 기능으로 간주하지 않는다. [기능 플래그](https://superset.apache.org/docs/configuration/feature-flags/)
- Snowflake Dynamic Tables의 target lag는 최선 노력의 신선도 목표이며 보장된 SLA가 아니다. 레이포트도 ‘설정한 갱신 주기’와 ‘현재 데이터의 실제 시점’을 구별해 표시한다. [target lag](https://docs.snowflake.com/user-guide/dynamic-tables-target-lag)
