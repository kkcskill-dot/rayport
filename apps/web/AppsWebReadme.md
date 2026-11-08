# 레이포트 React 1.0

Rust 서버의 `/api/v1` API를 사용하는 React + TypeScript 앱이다. 가상 값으로 로그인·조회·저장을 대신하지 않는다. 로그인은 서버가 발급하는 HttpOnly 세션 쿠키를 사용하고 접근 토큰은 localStorage/sessionStorage에 저장하지 않는다.

## 구현 범위

- 로그인, 세션 복구·만료 처리, 역할별 메뉴와 빈 상태·오류·중복 제출 제한.
- 업무 질문의 등록 도구 검색, 절대 날짜·점포 조건 확인, 제한 조회와 실제 결과 표·순매출 차트.
- 도구 초안 등록·수정, 낙관적 버전 검사, 검증 기록 제출, 게시·사용 중지, OpenAPI/MCP 도구 정의 JSON 내려받기.
- 생성 요청, 검토·구현·사용 준비 단계, 원래 질문 복귀, 요청자의 해결 확인. 다른 요청자의 해결을 관리자가 대신 확정하지 않는다.
- 관측 JSON 반입, ASH 표본/실행 수/자료 출처/관측 구간 구분, 후보에서 API 초안 생성.
- 마트 전체 스냅샷 반입과 명시적인 교체 확인, 버전·행 수·완료 영업일 표시.
- 실행 중지/재개, 사용자·원천 예산, 감사 기록, 종료 미확인 작업의 원천 종료 확인.

합성 자료는 예제 JSON을 내려받아 사용자가 명시적으로 반입할 때만 저장된다. 기존 정적 HTML 설계 시안은 별도 과거 산출물이며 현재 React 런타임과 동일한 구현이 아니다.

## 현재 기능 경계

질문 검색은 등록된 이름·키워드 기반이며 LLM 자유 분석은 연결되지 않았다. 원본 AWR HTML 직접 파싱, CDC·자동 적재·정기 마트 갱신, 실제 MCP 서버 연결도 프론트 구현 범위가 아니다. APIC는 서버의 연결·권한·취소 계약에 따라 실행된다. 프론트 역할 숨김이나 입력 검사는 서버의 권한·부하 검사를 대체하지 않는다.

KRW 금액은 원 단위 정수다. 브라우저의 안전 정수 범위를 벗어나는 금액은 부정확하게 표시하지 않고 응답을 거절한다. APIC 1.0 조회는 단일 점포를 명시하며 마트는 허용된 여러 점포를 조회할 수 있다. 종료일은 미포함이다.

## 계층과 계약

- `src/domain`: 프레임워크에 의존하지 않는 API/업무 타입.
- `src/application`: 순수 입력·상태 규칙, 명시 합성 예제, `RayportApi` 포트.
- `src/infrastructure/http`: HTTP 응답·오류 처리, same-origin 세션·CSRF 헤더.
- `src/presentation`: React 상태, 접근 가능한 폼, 페이지·표·차트.
- `src/bootstrap`: HTTP 어댑터를 구성해 앱에 주입.

application/presentation에서 infrastructure를 직접 import하지 않는다. 모든 소스·테스트 파일은 저장소 상대 경로의 PascalCase 접두사를 사용한다. `AppsWebViteConfig.ts`와 `AppsWebVitestConfig.ts`는 명령에서 명시한다.

API 응답은 `{ data: ... }`, 오류는 `{ error: { code, message } }`다. 모든 mutation은 `X-Rayport-Request: 1` 헤더를 보내며 쿠키는 same-origin에서만 사용한다. 프론트와 API를 같은 origin에서 제공해야 한다. 웹에서 인증키·DB 연결·SQL 실행 주소를 직접 설정하지 않는다.

## 검증

Node v24.22.2 환경에서 의존성을 설치하고 package-lock.json을 생성했다. TypeScript 검사와 Vite production 빌드, Vitest/JSDOM 테스트를 실행했다. 테스트는 메모리 DOM과 HTTP 포트 대체 객체를 사용하며 로컬 서버·DB를 시작하지 않는다. 실제 브라우저 렌더링, 실제 APIC/인증/원천 DB 통합은 별도 검증 범위다.

```sh
npm --prefix apps/web ci
npm --prefix apps/web test
npm --prefix apps/web run build
```

검증 사례: 로그인·세션 만료, 역할 메뉴, 미지원 질문의 요청 접수, 날짜 검증·조회 결과와 조건 변경 시 무효화, 요청자의 해결 확인, 초안 버전 충돌, 검증 전 게시 방지, 전체 스냅샷 교체 확인, 종료 미확인 실행 유지, CSRF 헤더, HTTP 오류·금액 정밀도.

production 산출물은 `apps/web/dist`다. 서버 패키지에 해당 디렉터리를 포함하고 Rust의 web_directory 설정과 일치시킨다. 개발 서버 기동은 별도 명시 요청이 있을 때만 수행한다.
