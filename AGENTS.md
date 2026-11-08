# Rayport 작업 지침

- 작업 시작 시 `/Users/user/obs/02 공통 기억/빠른 참조.md`와 관련 프로젝트 기억을 먼저 확인한다.
- 2026-11-08 후속 사용자 요청으로 1.0 구현·테스트·개선까지 확장했다. 현재 범위 정본은 docs/delivery/DocsDeliveryRayportV1Release.md다. 설계상 미래 기능을 구현 완료로 보고하지 않는다.
- Rust + React, 사내 서버 설치형. 원천 실행은 APIC를 활용한다. Spharos Gateway를 기본 경로에 넣지 않는다.
- 로컬 서비스·DB는 명시적인 기동 요청 없이 시작하지 않는다. 테스트가 임시 DB를 시작하는 경우도 포함한다.
- 운영 DB 접속, 실데이터 반입, 최초 적재, 실제 API 게시·배포는 현재 수행되지 않았다.
- UI는 한국어 중심. Pretendard 우선, 자간 -0.03em, 짝수 폰트 크기·최소 12px, 여러 줄 1.5 줄 높이.
- 민감한 SQL 원문, 바인드값, 데이터, 인증정보를 저장소·로그·공통 기억에 넣지 않는다. 예시는 합성 데이터를 쓴다.
- ASH 표본 수를 실행 횟수로 표현하지 않는다. 생성 API 후보는 승인된 실행 API와 구분한다.
- 캐시·마트·APIC 모든 경로에서 사용자 권한과 데이터 기준을 유지한다. 실패 시 운영 원천 자동 우회는 금지한다.
- 변경 결과, 검증 여부, 미완료 항목은 README/관련 문서와 Obsidian 프로젝트 메모에 기록한다.


## 구조와 파일명 (2026-11-08 사용자 요구)

- 저장소 상대 전체 디렉터리 경로를 PascalCase로 이어 붙여 파일명 접두사로 쓴다. 예: `aaaa/bbbb/cccc/AaaaBbbbCcccWork.file`. 의미 이름도 PascalCase, 최종 확장자는 소문자 한 개다.
- 파일명은 Unicode NFC·대소문자 비구분 비교에서도 유일해야 한다. 파일 이동 시 이름·import·진입 설정·문서 링크를 함께 수정한다.
- 고정 도구 파일 예외는 `tools/validation/ToolsValidationValidateRepository.py`의 정확한 경로 allowlist만 인정한다. 임의의 README·index·utils 이름을 각 폴더에 반복 생성하지 않는다.
- 구조의 정본은 `docs/architecture/DocsArchitectureRayportProjectStructure.md`다. 도메인은 순수 규칙, application은 유스케이스/포트, infrastructure는 어댑터, presentation은 표시, bootstrap은 의존성 조립을 담당한다.
- `python3 tools/validation/ToolsValidationValidateRepository.py`로 명명·충돌·상대 import·프론트 의존·문서 링크를 확인한다. 이 정적 스캔은 컴파일러/타입검사를 대체하지 않는다.
