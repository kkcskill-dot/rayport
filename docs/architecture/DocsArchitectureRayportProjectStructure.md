# Rayport 프로젝트 구조와 파일 명명 기준

작성일: 2026-11-08. 적용 범위: 이 저장소의 신규 파일·이동·분리·코드 생성. 제품 기능의 구현 완료를 뜻하는 문서는 아니다.

**초기는 Rust 모듈형 단일 서비스와 React 앱으로 구성하고, 업무 규칙을 실행 기술에서 분리한다. 파일명은 저장소 상대 경로를 PascalCase로 연결해 파일 하나만 보아도 소속과 책임을 알 수 있게 한다.** 디렉터리와 파일을 미리 많이 만드는 대신, 실제 기능을 구현할 때 정해진 경계를 따라 확장한다.

## 1. 구조를 판단하는 원칙

1. **업무 책임이 먼저다.** `catalog`, `discovery`, `query`, `request`, `mart`, `governance`를 구분하며 HTTP·React·APIC·PostgreSQL 같은 기술 이름으로 업무 모델을 정의하지 않는다.
2. **의존성은 안쪽으로 향한다.** 도메인은 화면·네트워크·DB를 모르고, 애플리케이션이 정의한 포트를 외부 어댑터가 구현한다.
3. **조립 지점은 한곳이다.** `bootstrap`만 실제 연결기·설정·서비스를 생성해 연결한다. 업무 함수가 전역 싱글턴에서 연결기를 꺼내지 않는다.
4. **소유자가 있는 계약을 사용한다.** 다른 문맥의 내부 테이블이나 파일을 직접 읽지 않고 공개 계약을 사용한다.
5. **구현된 경계를 검증한다.** crate 의존성·import 규칙·파일명 검사로 구조를 확인한다. 폴더 이름만 계층으로 나눠 놓고 자유롭게 참조하지 않는다.
6. **현재 범위와 목표를 구분한다.** 단일 상태 응답만 있는 서버에 미사용 repository·use case·DI framework·빈 crate를 만들지 않는다.

서버 설치형이라는 요구가 곧 여러 서비스나 Kubernetes를 의미하지 않는다. 독립 배포·보안 경계·장애 격리·확장 요구가 생겼을 때만 프로세스를 분리한다. 처음부터 모든 업무 문맥을 마이크로서비스로 나누지 않는다.

## 2. 현재 초기 범위와 목표 구조

**1.0 구현 갱신:** domain/application/adapters/server 네 crate가 실제로 구현되었다. application은 catalog/discovery/mart/request/query/governance 모듈로 나눴다. 아래 초기 구조 표와 `[목표]` 트리는 설계 당시 기록이며, 최신 구현은 [릴리스 기록](../delivery/DocsDeliveryRayportV1Release.md)과 실제 저장소를 따른다.

초기 구조 정리 당시의 실제 범위는 `apps/web`의 화면 시안, `crates/rayport-server`의 상태 응답용 셸, 설계 문서·합성 예제 계약, 저장소 검증 도구다. 실사용 인증·APIC 실행·DB 저장·ASH/AWR 분석·마트 갱신은 아직 구현된 기능이 아니다.

| 영역 | 이번 구조 | 실제 기능을 구현할 때 확장 |
|---|---|---|
| Rust | domain/application/adapters/server 네 crate, 명시적 진입점 | 데이터 규모에 따른 저장 어댑터 확장 |
| React | 부트스트랩·표현·앱 상태/시나리오·모델·예제 자료의 책임 분리 | 실제 API 포트·인증 컨텍스트·계약 생성 타입 |
| 계약 | 합성 매출 기능의 예시 JSON | 검증 가능한 기능 스키마·OpenAPI·MCP 산출물 |
| 데이터 | 실제 저장소 연결 없음 | 메타데이터 repository와 제공용 마트 adapter |
| 검증 | 파일 구조·명명 검사, 가능한 빌드·타입 검사 | 문맥 의존성·권한·경로·갱신 정합성 회귀 검사 |
| CI | 로컬 검증 명령과 기준 문서화 | 사용 중인 CI에 동일 명령 연결 |

현재 파일 배치의 정본은 실제 저장소다. 아래 트리에서 `[목표]`는 생성 예정 디렉터리이며, 빈 디렉터리·빈 파일을 만들어 구현된 것처럼 보이게 하지 않는다.

```text
rayport/
├── AGENTS.md                         작업 규칙·구조 변경 시 준수사항
├── README.md                         진입 문서·현재 구현/검증 상태
├── Cargo.toml                        Rust workspace
├── Cargo.lock                        생성 후 버전 고정
├── apps/
│   └── web/
│       ├── package.json              웹 의존성·명시적인 실행 명령
│       ├── tsconfig.json             타입 검사 설정
│       ├── index.html                Vite HTML 진입점
│       ├── AppsWebViteConfig.ts       이름을 지정해 사용하는 Vite 설정
│       ├── AppsWebReadme.md           웹 앱 안내
│       └── src/
│           ├── bootstrap/            AppsWebSrcBootstrapRayportWebMain.tsx
│           ├── presentation/         페이지·컴포넌트·스타일·접근성
│           ├── application/          사용자 흐름·상태 전이·포트
│           ├── domain/               화면이 이해하는 업무 모델·순수 규칙
│           └── infrastructure/       합성 자료·향후 HTTP adapter
├── crates/
│   ├── rayport-server/
│   │   ├── Cargo.toml
│   │   └── src/bootstrap/
│   │       └── CratesRayportServerSrcBootstrapRayportServerMain.rs
│   ├── rayport-domain/               [목표] 순수 업무 모델·정책 규칙
│   ├── rayport-application/          [목표] 유스케이스·포트·작업 경계
│   └── rayport-adapters/             [목표] APIC·저장소·LLM·파일 adapter
├── contracts/
│   ├── tools/
│   │   └── ContractsToolsStoreSalesExample.json
│   ├── schemas/                     [목표] 검증용 스키마
│   └── generated/                   [목표] 원본 계약에서 생성한 산출물
├── docs/
│   ├── product/DocsProductRayportProductDesign.md
│   ├── research/DocsResearchRayportCompetitiveAnalysis.md
│   ├── architecture/DocsArchitectureRayportProjectStructure.md
│   ├── architecture/DocsArchitectureRayportDataApiArchitecture.md
│   ├── ux/DocsUxRayportUxDesign.md
│   ├── contracts/DocsContractsRayportSalesContractAndValidation.md
│   ├── delivery/DocsDeliveryRayportDeliveryPlan.md
│   ├── prototypes/DocsPrototypesRayportPreview.html
│   └── adr/                         [목표] 중요한 결정의 근거·변경 이력
└── tools/
    └── validation/ToolsValidationValidateRepository.py
```

여기서 웹의 기능별 하위 폴더는 실제 파일의 책임이 생길 때 추가한다. 설계 문서에 표시된 목표 디렉터리는 현재 생성 여부와 별개다. 실행·설치·배포 명령은 README와 delivery 문서에서 관리하고, 구조 문서에 환경별 절차를 중복 복사하지 않는다.

## 3. 업무 문맥과 소유권

| 문맥 | 소유하는 상태·규칙 | 외부 문맥과 주고받는 계약 |
|---|---|---|
| `catalog` | 업무 기능, 의미·입출력 계약, 버전, 소유자, 게시 상태 | 실행 가능한 `PublishedCapability`와 계약 버전 |
| `discovery` | 반입 근거, SQL 군집, API/마트 후보, 근거 품질 | 후보 초안과 근거 참조; 게시 권한은 소유하지 않음 |
| `query` | 질문 해석 결과, 실행 계획, 결과 기준 시각, 실행 상태 | 승인 기능·권한 결정·원천 예산을 입력받아 실행 |
| `request` | 기능 생성 요청, 부족한 범위, 담당자, 원질문과 상태 이력 | 기존 기능 연결, 후보 생성 요청, 질문 해결 확인 |
| `mart` | 마트 정의, 빌드, watermark, 품질, 게시 snapshot, 수명주기 | 검증된 제공 경로와 snapshot 정보 |
| `governance` | 주체·유효 범위·정책 버전·누적 예산·감사 규칙 | 권한 결정과 실행/공개 예산 예약·확정 |

`governance`가 모든 문맥의 업무 정책을 소유하지는 않는다. 매출의 마감 정의는 해당 기능 계약, 마트의 품질 규칙은 `mart`, 권한·누적 범위와 공통 실행 예산은 `governance`가 소유한다. 분석 SQL 파서의 기술 구현은 adapter이며, 발견한 후보의 의미와 상태 전이는 `discovery`의 모델이다.

다른 문맥을 참조해야 하는 흐름은 application 유스케이스에서 조정한다. 예를 들어 기능 게시 유스케이스는 catalog 계약, discovery 근거, governance 승인 결과를 조합한다. 세 문맥이 서로의 내부 struct·DB table을 순환 참조하지 않는다. 공통 ID·시각·오류 분류처럼 실제로 공유되는 작은 값만 공통 모듈에 둔다. `utils`, `common`, `manager`를 책임이 없는 임시 보관소로 사용하지 않는다.

## 4. Rust 의존성과 헥사고날 구조

```text
rayport-server (bootstrap + HTTP 진입 adapter)
      │                         │
      ▼                         ▼
rayport-application ◀──── rayport-adapters
      │                         │
      └──────────┬──────────────┘
                 ▼
          rayport-domain
```

`rayport-domain`은 axum·reqwest·sqlx·Tokio·APIC DTO·React용 JSON을 의존하지 않는다. 데이터가 어떤 방식으로 저장되고 전달되는지 모르는 값 객체·불변식·상태 전이가 들어간다. transport serialization을 도메인에 강제로 넣지 않고 경계에서 DTO로 변환하는 것을 기본으로 한다.

`rayport-application`은 도메인과 함께 실행 흐름을 정의한다. `CapabilityRepository`, `SourceExecutionPort`, `ScopeBudgetPort`, `Clock`, `AuditPort`처럼 유스케이스가 필요로 하는 능력을 포트로 정의한다. 특정 라이브러리의 connection·transaction·HTTP response를 포트 반환 타입으로 사용하지 않는다. 시간·ID 생성·외부 실행도 대체 가능한 경계로 두되, 사용하지 않는 포트를 미리 만들지 않는다.

`rayport-adapters`는 APIC HTTP 호출, 파일 반입 parser, PostgreSQL 저장, LLM 연결, 외부 신원 검증 등을 구현한다. 외부 오류·DTO를 application이 정의한 의미로 변환한다. API timeout과 DB 실행 취소의 차이는 이 adapter 계약에 드러나야 하며, timeout을 정상 종료로 숨기지 않는다.

`rayport-server`는 설정을 읽어 adapter를 생성하고 유스케이스와 HTTP 핸들러를 연결한다. HTTP 핸들러는 요청 검증·인증 컨텍스트 수신·유스케이스 호출·응답 변환을 담당한다. `SELECT`, 매출 계산, 권한 범위 결정, 기능 게시 상태 전이가 핸들러에 들어가지 않는다.

초기 서버는 상태 확인 셸뿐이므로 위 세 라이브러리 crate를 즉시 생성하지 않는다. 첫 실제 기능의 모델·유스케이스·adapter를 구현할 때 필요한 crate를 만든다. worker 실행 수명주기가 API와 달라질 때 서버의 별도 `[[bin]]` 또는 독립 worker crate를 선택한다. 이때도 같은 application 유스케이스를 재사용한다.

| 참조하는 곳 | 허용 | 금지 |
|---|---|---|
| domain | 표준 라이브러리·선별한 순수 계산 의존성 | application·adapters·server·I/O framework |
| application | domain·유스케이스에 필요한 순수 지원 타입 | adapters·server·구체 DB/HTTP client |
| adapters | application 포트·domain 공개 값 | server·다른 adapter의 내부 구현 |
| server bootstrap | application·adapters·domain의 공개 계약 | 업무 규칙을 자체 구현해 우회 |
| server HTTP 진입부 | application 입출력·인증 컨텍스트 | 구체 저장소를 직접 조작 |

Cargo가 막는 것은 선언하지 않은 crate 참조와 순환 crate 의존성이다. 금지된 의존성을 누군가 Cargo.toml에 추가하는 것까지 자동으로 막지는 않으므로, 정적 검증에서 허용 crate 그래프도 검사해야 한다. 단일 crate 내부 단계에서는 `pub(crate)`·비공개 모듈·명시적인 공개 인터페이스로 노출을 줄인다.

## 5. React 책임 분리

| 계층 | 책임 | 허용 의존성 |
|---|---|---|
| `presentation` | 페이지·표·차트·카드·입력·접근성·시각 상태 | application, domain 읽기 모델, UI 라이브러리 |
| `application` | 질문 제출·조건 보완·요청 등록·화면 상태 전이·포트 | domain·순수 타입 |
| `domain` | 기능·후보·요청·결과의 업무 타입과 순수 규칙 | framework 없는 순수 코드 |
| `infrastructure` | HTTP client, 계약 DTO 변환, 합성 fixture, 저장소 adapter | application 포트, domain, 생성 계약 타입 |
| `bootstrap` | React root·의존성 주입·실행 모드·라우트 조립 | 모든 계층의 공개 인터페이스 |

presentation은 infrastructure를 직접 import하지 않는다. 컴포넌트에서 `fetch`를 호출하거나 전역 API client를 꺼내지 않는다. HTTP 실패를 사용자가 이해하는 상태로 바꾸는 작업은 application과 adapter 경계에서 수행한다. 반면 버튼 hover·dialog 열림처럼 업무와 무관한 일시적 UI 상태는 presentation에 둔다.

프런트엔드 domain은 서버의 최종 권한 판정을 복제하지 않는다. 화면에서 입력 조건을 안내할 수는 있어도, 사용자 권한·원천 부하·게시 승인은 서버에서 다시 판정한다. 프런트엔드 캐시에 허용 범위를 넘는 데이터를 먼저 받아 두고 화면만 가리는 방식은 금지한다.

기능이 늘면 각 계층 아래에 `catalog`, `discovery`, `query`, `request`, `mart`, `governance` 폴더를 필요한 만큼 만든다. 문맥 간 공유가 필요하면 application의 공개 계약을 거친다. 다른 문맥의 페이지를 임포트해 내부 state를 사용하는 구조는 피한다. 모든 폴더에 동일한 boilerplate를 복제하지 않는다.

코드상의 컴포넌트명·타입명·함수명은 `QuestionPage`, `ResolveQuestion`, `Capability`처럼 읽기 쉽게 유지할 수 있다. 길어진 파일명과 동일한 이름을 모든 심벌에 강요하지 않는다. 파일 소속은 경로·파일명으로, 코드의 의미는 언어 관례에 맞는 이름으로 표현한다.

화면 시안의 순수 상태·요청 생성 함수는 `application/AppsWebSrcApplicationRayportSession.ts`에 두고, React hook은 `presentation/bindings/AppsWebSrcPresentationBindingsUseRayportSession.ts`에 둔다. `application/ports`의 자료 포트는 유지하며 bootstrap이 합성 자료 adapter를 연결한다. 시안 단계에서도 application에 React 의존 예외를 만들지 않는다. 실제 인증·APIC·영속 저장이 구현된 것은 아니다.

## 6. 전체 경로를 사용하는 파일명

### 6.1 공식 규칙

저장소 루트 아래의 디렉터리 전체를 PascalCase로 변환해 이어 붙이고, 파일의 의미 이름을 PascalCase로 붙인다. 마지막 확장자는 소문자로 유지한다. 절대 경로와 저장소 폴더 이름 `rayport` 자체는 접두사에 포함하지 않는다.

```text
basename = PascalCase(repository-relative parent directories)
         + PascalCase(semantic file stem)
         + lowercase(final extension)

AAAA/BBBB/CCCC/WORK.FILE
→ AAAA/BBBB/CCCC/AaaaBbbbCcccWork.file
```

디렉터리는 소문자 kebab-case를 기본으로 한다. 기존 PascalCase 파일의 단어 경계, 하이픈·밑줄, 대문자 약어를 일관되게 토큰화한다. `API → Api`, `SQL → Sql`, `JSON → Json`, `UI → Ui`를 기본으로 하며 단어를 전부 대문자로 남겨 예외 이름을 늘리지 않는다. `rayport-server → RayportServer`다.

이미 전체 경로 접두사가 붙은 파일을 재검사할 때 접두사를 한 번 더 붙이지 않는다. 이동은 기존 접두사를 제외한 의미 이름을 유지하고 새 부모 경로로 다시 계산한다. 명명 검증기는 잘못된 이름을 보고하며 파일을 자동으로 이동하거나 수정하지 않는다.

| 배치 | 의미 이름 | 최종 파일명 |
|---|---|---|
| `apps/web/src/bootstrap/` | `main` | `AppsWebSrcBootstrapMain.tsx` |
| `apps/web/src/presentation/query/` | `question-page` | `AppsWebSrcPresentationQueryQuestionPage.tsx` |
| `apps/web/src/application/query/` | `resolve-question` | `AppsWebSrcApplicationQueryResolveQuestion.ts` |
| `apps/web/src/infrastructure/apic/` | `api-client` | `AppsWebSrcInfrastructureApicApiClient.ts` |
| `crates/rayport-server/src/bootstrap/` | `rayport-server-main` | `CratesRayportServerSrcBootstrapRayportServerMain.rs` |
| `crates/rayport-domain/src/catalog/` | `capability` | `CratesRayportDomainSrcCatalogCapability.rs` |
| `docs/architecture/` | `rayport-project-structure` | `DocsArchitectureRayportProjectStructure.md` |
| `contracts/tools/` | `store-sales.example` | `ContractsToolsStoreSalesExample.json` |
| `tools/validation/` | `validate-repository` | `ToolsValidationValidateRepository.py` |

복합 확장자처럼 보이는 `.example.json`, `.test.ts`는 마지막 확장자 앞부분을 의미 이름으로 옮긴다. 따라서 각각 `Example.json`, `Test.ts`로 정규화하고 테스트 도구의 탐색 패턴을 명시한다.

### 6.2 유일성과 이식성

PascalCase로 단어를 연결하는 것만으로 수학적인 유일성이 보장되지는 않는다. 서로 다른 경로 구분이 같은 basename을 만들 수 있으므로 **전체 basename 중복과 대소문자를 무시한 중복**을 검사한다. 충돌하면 책임을 구체적으로 나타내는 의미 이름으로 바꾸며 무의미한 번호로 숨기지 않는다.

macOS에서 실행된다는 이유로 import의 대소문자를 다르게 쓰지 않는다. Linux의 대소문자 구분 파일시스템에서도 같은 파일을 찾아야 한다. 대소문자만 바꾸는 이동은 임시 이름을 거쳐 Git 차이와 참조를 확인한다. 구현 파일은 ASCII 이름을 기본으로 하며 Unicode 정규화에 의존하지 않는다.

절대 경로·사용자 이름·장비 이름·날짜를 모든 파일명에 붙이지 않는다. 깊은 폴더는 파일명을 길게 만들므로 책임을 설명하지 않는 계층을 늘리지 않는다. 배포 대상 OS·압축 도구의 경로 길이 제한은 실제 패키징 단계에서 확인한다.

### 6.3 고정 이름 예외

도구가 탐색하는 이름은 **정확한 경로를 나열한 allowlist**로 예외 관리한다. 일반 소스에 광범위한 와일드카드 예외를 만들지 않는다.

- Rust의 `Cargo.toml`, `Cargo.lock`.
- Node의 `package.json`과 채택한 패키지 관리자의 lockfile.
- TypeScript의 `tsconfig.json`, Vite의 HTML 진입점 `index.html`.
- 저장소 루트의 `README.md`, `AGENTS.md`.
- Git 탐색 규약에 필요한 `.gitignore`는 Git 메타 설정으로 명시한다.

하위 앱의 안내문은 도구 고정 이름이 아니므로 `apps/web/AppsWebReadme.md`처럼 명명한다. `vite.config.ts`는 `--config`로 지정할 수 있어 예외가 아니다. Rust의 `main.rs`, `lib.rs`, `mod.rs`도 명시적인 path로 연결한다. 향후 CI 정의도 도구가 고정 basename을 요구하지 않으면 같은 규칙을 적용한다.

`.git/`, `node_modules/`, `target/`, `dist/`, 임시 캐시와 외부 의존성은 관리하는 소스의 검사 대상에서 제외한다. 실제 데이터·비밀정보는 저장소에 넣지 않는다. 검사 대상 제외와 고정 이름 예외는 구분한다. 직접 관리하는 생성 소스는 생성물이라는 이유만으로 무조건 제외하지 않는다.

## 7. Rust·Vite와 긴 파일명의 연결

Cargo는 `[[bin]]`, `[lib]`, `[[test]]`의 `path`로 소스를 지정할 수 있다. 내부 crate·module 이름은 Rust의 snake_case를 유지하고 물리 파일명과 논리 이름을 분리한다. [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html)

```toml
# crates/rayport-server/Cargo.toml 연결 예시
[[bin]]
name = "rayport-server"
path = "src/bootstrap/CratesRayportServerSrcBootstrapRayportServerMain.rs"
```

향후 domain crate를 만드는 경우의 예시:

```toml
[lib]
name = "rayport_domain"
path = "src/CratesRayportDomainSrcRayportDomainLib.rs"
```

```rust
// src/CratesRayportDomainSrcRayportDomainLib.rs
#[path = "catalog/CratesRayportDomainSrcCatalogCapability.rs"]
pub mod capability;
```

`#[path]`는 해당 소스의 위치에 대한 상대 경로로 관리한다. inline module 안에서는 경로 해석 규칙이 달라지므로 초기 규약에서는 파일 수준 선언을 사용하고 깊은 inline module에 상대 경로를 섞지 않는다. 공개 module 이름까지 파일명과 같이 길게 만들 필요는 없다. [Rust module path](https://doc.rust-lang.org/reference/items/modules.html)

Vite는 package.json의 관련 script에서 `--config AppsWebViteConfig.ts`를 명시하고 index.html의 module script도 새 bootstrap 파일로 연결한다. 개발·build·preview가 서로 다른 설정을 우연히 읽지 않게 한다. [Vite 설정 파일](https://vite.dev/config/)

TypeScript alias를 사용하면 `@presentation`, `@application`, `@domain`, `@infrastructure`처럼 책임을 나타낸다. alias 자체는 의존 경계를 강제하지 않는다. TypeScript의 `paths`는 실행 시 import를 바꾸지 않으므로 bundler와 test runner에도 같은 해석을 설정한다. 초기 파일 수에서는 명시적 상대 import를 사용해도 된다. [TypeScript paths](https://www.typescriptlang.org/tsconfig/paths.html)

## 8. 테스트·계약·생성물의 명명

테스트도 전체 경로 접두사를 사용한다. 예시는 `crates/rayport-application/tests/query/CratesRayportApplicationTestsQueryExecutionBudgetTest.rs`, `apps/web/tests/application/AppsWebTestsApplicationResolveQuestionTest.ts`다. Rust 통합 테스트는 필요할 때 `[[test]]`에 짧은 snake_case 이름과 명시 path를 등록한다. 자동 탐색과 수동 등록의 중복이 생기면 해당 자동 탐색을 비활성화한다.

테스트는 계층 책임에 맞춘다. domain은 불변식·상태 전이, application은 fake port를 통한 업무 시나리오, adapter는 계약·변환·실제 취소 의미, presentation은 사용자 조작·표시를 검증한다. 단순 이름 변경 때문에 구현을 그대로 반복하는 테스트를 만들지 않는다. 명시적인 기동 허용 없이 로컬 DB·서버를 자동으로 시작하는 테스트를 실행하지 않는다.

기능 계약의 정본을 하나로 정한다. 기능 정의·입출력 스키마에서 OpenAPI·MCP tool schema·Rust/TypeScript 경계 타입을 생성한다면 원본 version/hash와 생성기 version을 기록한다. 생성물은 직접 수정하지 않는다. 생성기 또는 출력 후처리에 명명 규칙을 적용하고 참조·import까지 함께 맞춘다.

서버 도메인 타입과 API DTO는 책임이 다르다. 타입을 기계적으로 공유하기 위해 도메인에 전송 형식의 제약을 넣지 않는다. 생성 계약과의 변환을 adapter에 두어 업무 의미와 공개 형식을 독립적으로 변경할 수 있게 한다.

DB migration 도입 시에는 `crates/rayport-adapters/migrations/CratesRayportAdaptersMigrationsV0001CreateCapability.sql`처럼 전체 경로 이름을 사용한다. migration 도구가 숫자 접두사 등 고정 이름을 요구하면 도구 선택 시 명시 manifest·설정으로 해결 가능한지 확인한다. 확인하지 않은 도구와 명명 규칙이 호환된다고 단정하지 않는다. 순서·checksum을 명시적으로 관리하며 운영 환경에 적용한 migration을 개명하거나 덮어쓰지 않는다.

## 9. 의존 규칙의 검증

구조 검사 진입점은 `tools/validation/ToolsValidationValidateRepository.py`다. CI 제품과 무관한 로컬 명령을 먼저 제공하고 향후 CI에서 같은 명령을 실행한다. CI workflow가 없으면 CI 강제가 완료되었다고 보고하지 않는다.

현재 검증기는 파일명·casefold 중복·JSON·상대 import·프런트엔드 계층 참조·로컬 문서 링크를 검사한다. 정적 문자열 스캔이므로 compiler나 AST 기반 import 분석을 대신하지 않는다. 각 확장 검증은 실제 구현 시 별도로 추가한다.

| 검사 | 목표 기능 | 구현 시 유의점 |
|---|---|---|
| 명명 | 전체 경로 prefix, 확장자, 예외 allowlist, basename 중복 | 기존 prefix 이중 부여 방지, 생성·의존 폴더 구분 |
| Rust 대상 | Cargo 명시 path 존재, 허용 crate graph | `cargo metadata`와 manifest 대조; 목표 crate를 실재로 취급하지 않음 |
| React 참조 | import 존재·대소문자·금지 계층 참조 | 동적 import·alias·재export까지 분석 범위 확장 |
| 문맥 경계 | 다른 문맥 내부 참조·순환 의존성 | 공개 계약과 내부 구현을 식별 가능한 배치로 분리 |
| 재현성 | lockfile·버전 고정·생성 차이 | 검증 중 서비스를 시작하지 않음 |
| 문서 참조 | 이동한 로컬 링크 누락 | 실제 파일 참조와 목표 경로 예시 구분 |

단순 텍스트 스캔으로 모든 import 경계를 보장했다고 주장하지 않는다. 현재 검증기가 지원하는 구문과 검사 범위를 명시하고, TypeScript compiler API 등의 AST 검증은 필요한 단계에서 추가한다. alias나 barrel export를 통해 경계 위반을 우회할 수 없는지도 확인한다.

서비스를 시작하지 않는 표준 검증 명령 예시:

```sh
python3 tools/validation/ToolsValidationValidateRepository.py
cargo fmt --all -- --check
cargo check --workspace
npm --prefix apps/web run typecheck
npm --prefix apps/web run build
```

이는 검증 절차이며 모든 명령의 성공 보고가 아니다. compiler·의존성이 없는 환경이면 실제로 확인한 범위를 README와 작업 기록에 남긴다. `cargo test`나 웹 테스트는 의미 있는 테스트를 추가한 시점부터 사용하고 DB 기동을 수반하지 않는 실행 범위를 명확히 한다.

## 10. 문서·ADR·이동 관리

| 정보 | 정본 |
|---|---|
| 작업 시 필수 규칙 | AGENTS.md |
| 현재 진입점·실행 방법·검증 결과 | README.md와 delivery 문서 |
| 구조·의존성·파일명 규칙 | 이 문서 |
| 업무 데이터·API 의미와 실행 방침 | 데이터/API 아키텍처 문서와 기능 계약 |
| 설계 판단의 이유·대안·변경 조건 | ADR |
| 실행 가능한 입출력 정의 | 기능 스키마 정본과 생성 산출물 |

ADR은 `docs/adr/DocsAdr0001ModularMonolith.md`처럼 전체 경로 prefix와 식별 번호·판단 이름을 포함한다. 상태·배경·결정·대안·영향·재검토 조건을 남긴다. 번호가 파일명 맨 앞에 있지 않다는 이유로 예외를 만들지 않는다. 설계를 채택했을 때 ADR을 작성하고 미결정안을 확정처럼 표현하지 않는다.

구조 변경 순서:

1. 대상 파일의 책임과 새 위치를 정하고 이전 경로→새 경로 대응을 만든다.
2. 이동한 부모 경로에 맞춰 파일명을 다시 계산한다.
3. import, Rust `#[path]`와 Cargo target, HTML 진입점, script, 계약 참조, 문서 링크를 같은 변경에서 수정한다.
4. 명명·의존 방향·타입·build를 검사하고 남은 이전 경로 참조를 확인한다.
5. README와 필요한 ADR에 변경 이유·검증 결과를 남긴다.

호환성을 이유로 이전 소스의 복사본을 남기지 않는다. 공개 API의 호환 adapter가 필요하면 그 책임과 폐기 조건을 명시한다. 구조 이동과 업무 사양 변경을 동시에 해야 할 이유가 없으면 별도로 검토할 수 있는 변경으로 나눈다.

## 11. 다음 확장의 시작 조건

첫 실제 기능은 승인된 기능의 APIC 실행 또는 기능 생성 요청 저장처럼 처음부터 끝까지 검증 가능한 작은 단위를 선택한다. 그 기능에 필요한 domain/application/adapter만 구현하고 다음 조건을 만족한 뒤 확장한다.

- 업무 규칙을 소유하는 문맥과 공개 계약이 명확하다.
- UI가 APIC·DB를 직접 참조하지 않는다.
- domain/application의 업무 핵심이 구체적인 I/O 기술을 참조하지 않는다.
- 파일명으로 소속 경로를 식별할 수 있고 중복이 없다.
- 데이터 권한·원본 보호·실행 취소·실패 상태가 계약에 드러난다.
- 구현됨·합성 자료·설계만 있음·미검증을 화면과 문서에서 구분한다.

이번 구조 정리는 로컬 서비스·DB 기동, 실제 자료 적재, APIC에 실제 API 게시, 고객 환경 배포를 포함하지 않는다. 구조 정비와 실운영 가능성은 서로 다른 완료 조건으로 관리한다.
