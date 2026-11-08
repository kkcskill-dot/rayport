# Rayport · 레이포트 1.0.0

기존 업무 SQL을 검토 가능한 API 후보로 만들고, 승인된 매출 도구와 마트 조회를 현업 질문·생성 요청에 연결하는 서버 설치형 Rust + React 제품의 첫 구현이다.

**현재 릴리스 범위는 단일 서버·단일 업무 공간·파일 저장소·제한된 일별 점포 매출 모델이다.** 실제 고객 APIC·Oracle 인수, Linux 배포, 범용 자연어 분석, SSO, 정기 적재는 완료된 기능이 아니다. 설치·실행과 외부 연동 검증을 구분한다.

## 구현된 흐름

- 토큰 기반 로그인, HttpOnly 세션, 관리자/분석가/조회자 권한, 서버의 점포 범위 강제.
- 업무 도구 초안 등록·편집·낙관 버전 검사·검증 근거·게시·폐기. OpenAPI/MCP 도구 정의 내려받기(실제 MCP 전송 서버 등록과 별개).
- 승인된 AWR/ASH 추출 템플릿 JSON의 정적 SQL 검사·정규화·군집화·API 후보화. 출처·관측 기간·실행 횟수와 ASH 표본을 구분. 원본 AWR HTML/PDF 파서나 리터럴 자동 치환기는 아니다.
- 마트 완전 스냅샷 반입, 검증 후 원자적 게시, 동일 스냅샷 재반입 멱등 처리, 최근 두 버전 유지, 기간×점포 완전성·신선도·정확한 정수 금액 검증.
- 등록 검색어에 의한 질문 매칭과 명시적인 기간·점포 선택 조회. 지원하지 않는 질문의 생성 요청·검토·개발·게시 연결·원래 질문 재조회·요청자 해결 확인.
- 사용자/원천 호출 제한, APIC 전체 공동 예산, 원자적 실행 예약, 중지 정책, 미확인 실행 슬롯 유지, 종료 확인, 감사 기록.
- HTTP APIC 어댑터 계약 구현: 승인 SQL/계약 hash, 신원·범위·입력·종료 확인. 실제 APIC가 이 별도 계약을 구현하고 인수 검증을 통과해야 연결 가능하며 기본 비활성이다.
- 체크섬·독점 잠금·원자적 파일 저장·실패 시 차단, 설치·업데이트·롤백·백업 절차.

## 코드 구조

```text
apps/web/src/               domain · application · infrastructure · presentation · bootstrap
crates/rayport-domain/       순수 값·권한·날짜·금액·SQL 안전 정책
crates/rayport-application/  catalog · discovery · mart · request · query · governance
crates/rayport-adapters/     단일 노드 파일 스냅샷 저장
crates/rayport-server/       HTTP·인증·APIC 어댑터·의존성 조립·CLI
contracts/                  업무 계약 예시
tools/validation/           명명·중복·계층·링크 검사
tools/release/              패키징·설치·설치 회귀 검사
docs/                       제품·설계·검증·운영 기록
```

전체 경로 접두사를 가진 유일 파일명을 사용한다. 예: `apps/web/src/domain/AppsWebSrcDomainRayportTypes.ts`. 고정 도구 진입 파일은 검사기의 정확한 경로 목록만 예외다. 파일 이동 시 이름·import·진입 설정·문서 링크를 함께 바꾼다.

## 확인과 실행 순서

1. [1.0 범위·검증 기록](docs/delivery/DocsDeliveryRayportV1Release.md)을 먼저 확인한다.
2. [설치·인증·업데이트·복구 절차](docs/delivery/DocsDeliveryRayportV1Operations.md)에 따라 패키지를 설치한다.
3. 초기화 후 설정의 점포 권한과 웹 경로를 지정하고 `check`를 실행한다. 초기 관리자도 점포 권한을 부여하기 전에는 매출을 조회할 수 없다.
4. 실제 기동은 운영자가 `serve --config PATH`를 명시적으로 실행한다. 이번 개발 작업에서는 서버·DB를 기동하지 않았다.
5. 로그인 후 합성 JSON 예제를 내려받아 마트/후보 반입→도구 검증·게시→조회→요청 해결을 시험한다. 실제 고객 자료와 합성 자료를 구분한다.

## 서비스 기동 없는 검증

이 Mac에서는 공식 배포본으로 설치한 사용자 전용 도구 체인을 사용했다. 다른 개발/CI 환경은 Rust·Node를 자체 준비한다.

```sh
source /Users/user/.local/share/rayport/toolchains/environment.sh
python3 tools/validation/ToolsValidationValidateRepository.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/web ci
npm --prefix apps/web test
npm --prefix apps/web run build
python3 tools/release/ToolsReleaseInstallTests.py
```

Node 24.22.2와 Rust 1.99.1을 사용했으며 Cargo.lock/package-lock.json을 포함한다. Mac의 로컬 SDK 호환 설정은 도구 체인 환경 파일 안에만 있으며 전역 설정은 변경하지 않았다.

## 설계 문서

- [제품·차별화](docs/product/DocsProductRayportProductDesign.md), [경쟁 비교](docs/research/DocsResearchRayportCompetitiveAnalysis.md)
- [프로젝트 구조·파일명](docs/architecture/DocsArchitectureRayportProjectStructure.md), [데이터·API](docs/architecture/DocsArchitectureRayportDataApiArchitecture.md)
- [UX](docs/ux/DocsUxRayportUxDesign.md), [매출 계약·인수 기준](docs/contracts/DocsContractsRayportSalesContractAndValidation.md)
- [초기 설계와 구현 범위의 관계](docs/delivery/DocsDeliveryRayportDeliveryPlan.md), [APIC 통합 계약](docs/contracts/DocsContractsRayportApicIntegration.md)

초기 [HTML 시안](docs/prototypes/DocsPrototypesRayportPreview.html)은 합성 자료를 쓰는 설계 기록이다. 현재 제품은 `apps/web`의 실제 HTTP 연동 앱이며 시안 숫자를 운영 데이터로 사용하지 않는다.
