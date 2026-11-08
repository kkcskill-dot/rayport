# Rayport 1.0 설치·운영·복구

이 문서는 1.0의 파일 기반 단일 노드 구성과 실제 CLI를 기준으로 한다. 설계 문서의 장기 PostgreSQL·CDC·SSO·OLAP 계획을 현재 지원 기능으로 해석하지 않는다. 패키징과 설정 검사는 서비스·DB를 기동하지 않는다. 실제 기동은 운영자가 별도로 `serve`를 실행할 때만 일어난다.

## 1. 지원 범위와 배포 전제

- Rust 바이너리와 빌드된 React 정적 파일을 함께 설치한다. Node/npm은 빌드 환경에 필요하고, 배포 서버의 웹 제공에는 필요하지 않다.
- 저장소는 단일 프로세스가 소유하는 로컬 파일시스템 스냅샷이다. DB 서버를 설치하지 않는다. 원자적 rename, advisory lock, 파일·디렉터리 fsync가 정상 동작하는 Unix 파일시스템이 필요하다.
- NFS/공유 볼륨, 다중 활성 노드, 외부 프로그램의 snapshot 직접 수정은 지원하지 않는다. 저장 디렉터리는 서비스 계정 소유 700, 파일은 600으로 관리한다.
- snapshot 파일은 최대 64MiB다. 초기 마트는 전체 교체 방식이며 마트당 최대 20,000행, 전체 최신 마트 합계 최대 100,000행, 최근 완성본 2개를 유지하는 제한을 따른다. 대규모 분석 저장소로 사용하지 않는다.
- 단일 테넌트·구성 파일 기반 사용자와 토큰 인증을 제공한다. OIDC/SSO, 중앙 비밀관리 연동, 다중 노드 세션 공유는 별도 통합 범위다.
- APIC 원천 실행은 `rayport-apic-v1` 사용자 정의 통합 계약을 구현하는 어댑터가 있어야 한다. 현행 고객 APIC의 기본 API와 바로 호환된다고 검증한 상태가 아니다. APIC 연결이 없으면 원천 실행은 비활성이다.
- 서버의 제한된 질문 매칭·업무 계약과 승인 API를 활용한다. 임의 자연어를 범용 SQL로 바꾸거나 AWR 원문 전체에서 업무 의미를 확정하는 제품으로 설명하지 않는다.

## 2. 빌드와 패키지

저장소 루트에서 실행한다. 이 명령은 컴파일·정적 자산 빌드·파일 조립만 수행한다.

```sh
bash tools/release/ToolsReleasePackage.sh
```

스크립트는 구조 검사, `cargo build --release -p rayport-server --locked`, `npm --prefix apps/web ci`, 프론트 빌드를 수행한다. 이미 해당 산출물의 테스트와 release 빌드를 마쳤다면 다음으로 조립만 할 수 있다.

```sh
bash tools/release/ToolsReleasePackage.sh --no-build
```

설치기의 회귀 검사는 서비스나 DB 없이 임시 합성 파일로 수행한다. 설치·업데이트·롤백·데이터 보존·잠금·변조와 경로 탈출 거절을 검사하며 실제 운영 설치 인수를 대체하지 않는다.

```sh
python3 tools/release/ToolsReleaseInstallTests.py
```

산출물은 `dist/rayport-1.0.0-<native-target>/`, 같은 이름의 `.tar.gz`, `.tar.gz.sha256`이다. 압축 내부에 바이너리·웹·문서·공개 계약 예제·설치 스크립트·`manifest.json`·`SHA256SUMS`가 있다. `data`, 실행 설정, 로그인 토큰, `.env`, 소스의 `node_modules`는 포함하지 않는다. 검증 체크섬은 손상 검출 수단이며 배포자 서명을 대신하지 않는다. 신뢰된 사내 배포 경로를 사용한다.

빌드 타깃은 현재 host로 제한하며 binary 형식·아키텍처를 확인한다. macOS에서 생성한 `aarch64-apple-darwin` 패키지는 Linux 배포본이 아니다. Linux 운영은 해당 Linux/CPU의 빌드 runner에서 다시 만들고 libc·인증서·파일시스템 동작까지 검증한다. Linux 빌드를 수행하지 않았다면 검증 완료로 기록하지 않는다.

이 Mac에 설치한 도구 체인은 `/Users/user/.local/share/rayport/toolchains/environment.sh`에 있다. macOS 패키징은 이 파일이 존재하면 현재 셸에서만 읽는다. 설치된 linker와 맞지 않는 SDK 27 대신 확인된 SDK 26.5를 선택하며 전역 SDK·프로필을 바꾸지 않는다. Linux에서는 이 파일을 읽지 않는다.

`SOURCE_DATE_EPOCH`을 지정하면 archive의 시간·정렬·uid/gid 메타데이터를 고정할 수 있다. 동일 입력 바이너리/웹 자산의 패키징을 안정화하는 기능이며 컴파일 결과 전체의 재현성을 보장하는 것은 아니다.

## 3. 첫 설치

패키지를 대상 서버로 전달하고 checksum을 확인한 뒤 압축을 푼다. 아래 `<native-target>`과 경로는 실제 패키지·서버 경로로 바꾼다. 설치·초기화만으로 서버가 시작되지는 않는다.

```sh
tar -xzf rayport-1.0.0-<native-target>.tar.gz
bash rayport-1.0.0-<native-target>/ToolsReleaseInstall.sh \
  rayport-1.0.0-<native-target>.tar.gz /opt/rayport
/opt/rayport/current/rayport-server init --directory /opt/rayport/instance
/opt/rayport/current/rayport-server check --config /opt/rayport/instance/config.json
```

`/opt/rayport`는 서비스 계정이 쓸 수 있도록 운영자가 준비한다. 설치기는 시스템 계정·systemd·TLS·방화벽을 만들거나 변경하지 않는다. 압축 파일 대신 검증된 압축 해제 디렉터리를 첫 인자로 줄 수도 있다. 설치기는 checksum·전체 파일 목록·플랫폼을 확인하고 경로 탈출·심볼릭 링크·특수 파일이 포함된 archive를 거절한다.

```text
/opt/rayport/
├── .install.lock                        설치 작업 직렬화 잠금
├── releases/rayport-1.0.0-<target>-<content-hash>/
├── current -> releases/<현재 버전>
├── previous -> releases/<이전 버전>        업데이트 후 생성
└── instance/                            업데이트 시 보존
    ├── config.json                      600
    ├── initial-admin-token.txt          초기 로그인 비밀, 600
    ├── web -> ../current/web
    └── data/                            최초 serve 때 생성, 700
        ├── snapshot.json
        └── .rayport.lock
```

`init`은 설정과 초기 관리자 토큰을 별도 파일에 생성한다. 이미 있는 설정이나 토큰 파일을 덮어쓰지 않으며 토큰 값을 콘솔에 출력하지 않는다. 관리자는 보호된 파일에서 토큰을 확인해 로그인하고, 승인된 비밀 저장소에 보관한 뒤 불필요한 초기 토큰 사본을 제거한다. 토큰을 티켓·채팅·Git·로그에 붙여넣지 않는다.

초기 관리자의 `store_ids`는 빈 배열이다. 관리자 역할은 모든 업무 데이터를 자동으로 조회할 권한을 뜻하지 않는다. 실제 허용 점포 ID를 서버 설정에 넣어야 해당 데이터 조회가 가능하다.

`check`는 설정의 형식·값을 검사한다. 데이터 snapshot 무결성, 웹 파일, APIC 접속, 외부 DB 취소를 검증하는 명령이 아니다. `serve` 단계에서 웹 파일·snapshot·테넌트 상태·원천 설정 등을 추가 확인한다.

## 4. 기본 설정과 명시적 기동

| 설정 | 기본값 또는 의미 |
|---|---|
| `bind` | `127.0.0.1:8080` |
| `public_origin` | `http://127.0.0.1:8080`, 정확한 scheme·host·port |
| `secure_cookie` | loopback HTTP에서는 false. HTTPS 운영에서는 true |
| `tenant_id` | 설치 초기 `local`. 저장된 workspace 테넌트와 같아야 함 |
| `data_directory` | instance 아래 data. 심볼릭 링크 없이 전용 로컬 저장소 사용 |
| `web_directory` | instance/web. 설치기의 current/web 링크를 사용 |
| `users` | 주체 ID·역할·테넌트·허용 점포와 토큰 SHA256 |
| `apic` | 기본 null. 명시적으로 구성한 경우만 원천 연결 활성화 |

외부 사용자는 사내 HTTPS 역방향 프록시를 통해 접속하게 한다. backend는 loopback 또는 보호된 내부 인터페이스에 두고, `public_origin`은 브라우저가 실제 접속하는 HTTPS origin과 정확히 일치시킨다. HTTPS이면 `secure_cookie: true`가 필요하다. backend의 HTTP 포트를 외부에 직접 노출하지 않는다. TLS 인증서·로그 수집·계정 관리는 고객 운영 표준에 따른다.

운영자가 실제 실행하기로 한 시점의 명령은 다음과 같다. 이 문서 작성·패키징 과정에서는 실행하지 않는다.

```sh
/opt/rayport/current/rayport-server serve --config /opt/rayport/instance/config.json
```

서비스 관리자를 연결할 때 동일 바이너리·설정 경로·전용 계정을 사용한다. 자동 서비스 등록 파일은 현재 패키지에 포함하지 않는다. 배포 서버에서 종료 신호에 대한 graceful shutdown 동작을 확인하고, 중지 명령 뒤 프로세스가 완전히 종료됐는지 확인해야 한다. 파일 잠금은 중복 프로세스 실행을 방지하며 lock 파일 자체를 지워 해제하지 않는다.

정상 확인 기준은 로그인 후 허용된 도구·마트를 조회할 수 있고, 권한 밖 점포가 거절되며, 데이터 기준 시각이 표시되고, 종료·재실행 후 요청/도구/마트 상태가 유지되는 것이다. `/healthz`·`/readyz` 응답만으로 원천 접속이나 실제 업무 수치의 정합성이 확인됐다고 보지 않는다.

## 5. 사용자 추가·토큰 교체·폐기

1. 서비스 중지와 종료를 확인하고 `config.json`의 보호된 백업을 만든다.
2. `users` 항목에 고유 `principal.id`, 동일 tenant, `admin`/`analyst`/`viewer` 중 역할, 실제 `store_ids`를 지정한다.
3. 최소 32바이트 암호학적 난수 토큰을 생성해 원문은 600 파일이나 비밀 저장소로만 전달한다. 설정에는 토큰의 소문자 SHA256 64자리만 넣는다. 예측 가능한 임의 문자열을 토큰으로 사용하지 않는다.
4. `check` 성공 후 운영자가 재기동한다. 구성은 자동 재적재되지 않는다.
5. 토큰 폐기는 사용자를 제거하거나 해시를 교체하고 재기동한다. 메모리 세션도 이때 폐기된다. 마지막 관리자 계정을 없애지 않는다.

브라우저 세션은 서버 메모리에 보관되며 8시간 만료, HttpOnly·SameSite=Strict 쿠키를 사용한다. HTTPS 배포는 Secure가 추가된다. 세션은 다중 노드 공유가 아니며 재기동 시 다시 로그인해야 한다. 토큰은 브라우저 저장소에 영속 저장하는 방식으로 운영하지 않는다.

다음은 새 토큰을 콘솔에 노출하지 않고 지정된 개인 파일과 SHA256 파일을 만드는 예다. 기존 파일은 덮어쓰지 않는다. 생성된 해시를 해당 사용자의 `token_sha256`에 반영하고 `check`를 수행한다.

```sh
python3 - /opt/rayport/instance/new-user-token.txt <<'PY'
from pathlib import Path
import hashlib, os, secrets, sys
path = Path(sys.argv[1])
token = secrets.token_hex(32)
for file, text in ((path, token), (path.with_suffix('.sha256'), hashlib.sha256(token.encode()).hexdigest())):
    fd = os.open(file, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'w') as output:
        output.write(text + '\n')
        output.flush()
        os.fsync(output.fileno())
print('Private token and hash files created; no credential printed.')
PY
```

## 6. APIC 연결 인수

`apic`은 `execute_url`, `cancel_url`, `status_url`, `token_env`, `timeout_seconds`, `contract_version`으로 구성한다. URL은 모두 HTTPS이며 URL 내부의 인증정보·query·fragment를 허용하지 않는다. `contract_version`은 `rayport-apic-v1`, timeout은 1~30초다. `token_env`에는 비밀값 대신 런타임 환경변수 이름만 넣는다. 서비스 관리자의 비밀 주입 기능으로 실제 값을 전달한다.

통합 어댑터는 승인 도구 ID·레이포트 계약 버전·SQL fingerprint/계약 hash·실행 ID·사용자/테넌트·서버가 계산한 허용 점포·제한된 입력을 받아야 한다. 원격은 DB 실행 전에 등록된 binding과 hash를 확인해야 하며 값을 그대로 echo하는 것은 검증이 아니다. 레이포트 버전은 APIC 네이티브 버전과 구분한다. 상세 wire 형식은 [APIC 통합 계약](../contracts/DocsContractsRayportApicIntegration.md)을 따른다. SQL을 사용자 입력으로 즉석 실행하는 경로를 만들지 않는다. 업무 조회의 데이터 형식과 금액 단위도 계약대로 반환해야 한다.

배포 전 실제 환경에서 권한 강제, 입력 바인딩, 결과 스키마/범위, HTTP timeout 이후 DB 실행 취소와 종료 확인을 검증한다. 취소/상태 확인이 불명확하면 해당 실행은 원천 예산을 계속 차지하며 운영자가 확인해야 한다. 장애를 해결하려고 APIC를 우회해 운영 DB에 직접 연결하거나 동일 실행을 맹목적으로 재시도하지 않는다.

AWR/ASH는 적법한 수집 권한·사용권과 승인된 반입 자료를 전제로 한다. ASH 표본 수와 AWR 실행 통계를 혼동하지 않는다. 정규화된 관측 자료에서 생성한 후보는 업무 의미와 권한·부하 검증 전까지 운영 API가 아니다.

## 7. 업데이트와 롤백

업데이트는 **서비스 중지 → 종료 확인 → instance 백업 → 패키지 검증/교체 → 설정 확인 → 운영자 재기동 → 업무 인수** 순서다. `--service-stopped`는 운영자가 중지를 완료했다는 명시 표시이며 설치기가 실제 프로세스를 강제로 중지하지 않는다.

```sh
bash <새 패키지>/ToolsReleaseInstall.sh <새 패키지>.tar.gz /opt/rayport --service-stopped
/opt/rayport/current/rayport-server check --config /opt/rayport/instance/config.json
```

설치기는 검증된 release 디렉터리를 추가하고 `current`를 원자적으로 전환하며 이전 release를 `previous`에 보존한다. `instance/config.json`, 토큰, data는 삭제·덮어쓰기하지 않는다. 기존 `instance/web`가 설치기의 관리 링크가 아니면 임의로 삭제하지 않고 설치를 거절한다. 사용자 정의 배치에서는 web 경로를 먼저 명시적으로 정리한다.

이전 버전으로 돌아갈 때도 서비스를 중지하고 종료를 확인한다. 동일 저장 schema와 데이터 모델에 호환되는 버전임을 확인한 뒤 다음처럼 이전 디렉터리를 설치 입력으로 사용한다.

```sh
bash /opt/rayport/current/ToolsReleaseInstall.sh /opt/rayport/previous /opt/rayport --service-stopped
/opt/rayport/current/rayport-server check --config /opt/rayport/instance/config.json
```

이 명령은 `current`/`previous`를 바꾸며 업무 데이터를 과거 시점으로 되돌리지 않는다. 저장 schema나 의미 계약이 변경된 릴리스는 바이너리만 롤백하지 말고 그 버전에 맞는 백업/마이그레이션 절차를 사용한다. 미래 버전의 자동 마이그레이션이 이미 구현돼 있다고 가정하지 않는다.

## 8. 백업·복구와 저장 오류

일관된 백업은 서비스 중지·종료 후 `instance/config.json`과 `instance/data`를 같은 세트로 복사한다. release 버전·manifest·시점도 함께 기록한다. 초기 토큰 파일의 추가 복사 여부는 비밀 관리 정책에 따르며 일반 문서 저장소로 올리지 않는다. 백업은 암호화·접근 통제된 별도 저장소에 보관한다.

복구는 신뢰된 백업을 별도 빈 디렉터리에 해제한 뒤 checksum과 설정을 확인한다. 아래 검사는 snapshot envelope 형식·체크섬만 확인하며 업무 수치나 APIC 연결을 검증하지 않는다.

```sh
python3 - /복구-검증-경로/data/snapshot.json <<'PY'
import hashlib, json, sys
from pathlib import Path
raw = Path(sys.argv[1]).read_bytes()
assert len(raw) <= 64 * 1024 * 1024, 'Snapshot exceeds supported size'
envelope = json.loads(raw)
assert envelope['schema'] == 1, 'Unsupported snapshot schema'
assert hashlib.sha256(envelope['payload'].encode()).hexdigest() == envelope['sha256'], 'Checksum mismatch'
payload = json.loads(envelope['payload'])
assert payload['schema_version'] == 1, 'Unsupported workspace schema'
print('Snapshot format and checksum valid; business reconciliation remains required.')
PY
```

검증 후에도 현재 데이터를 즉시 삭제하지 않는다. 서비스 종료 상태에서 현재 instance를 보존하고 검증된 백업으로 교체한다. 복구 위치가 달라졌다면 `data_directory`·`web_directory`의 절대 경로를 조정하고 사용자/tenant와 snapshot의 tenant가 같은지 확인한다. 설정 검사 후 명시적으로 기동해 대표 지표·요청·권한을 대조한다.

파일 변경은 임시 snapshot 작성·fsync·rename·디렉터리 fsync 순서다. rename 이후 오류는 commit 여부가 불확실하므로 프로세스 내 저장소가 읽기·쓰기를 모두 차단한다. 로그와 디스크 상태를 확인하고 명시적으로 종료·재개방한다. 손상/미지원 schema를 새 빈 상태로 자동 초기화하지 않는다. `.rayport.lock`을 지우거나 실패한 데이터 파일을 임의 삭제해 기동시키지 않는다.

서버 재기동 시 미완료 원천 실행은 재확인 대상으로 남긴다. 재기동했다고 실제 DB 실행이 종료된 것으로 간주하지 않는다. 파일 snapshot 백업이 원천 DB의 시점 일관성이나 마감 데이터를 대신 보장하지 않는다.

## 9. 운영 투입 전 완료 기준

1. 대상 OS/CPU에서 패키지 checksum·컴파일·실행·파일 잠금·복구 확인.
2. HTTPS origin·쿠키·인증·점포 권한과 권한 밖/재기동 후 세션 처리 확인.
3. 고객 APIC 어댑터의 계약·실취소·예산·실패·재시도 결과 확인.
4. 실제 업무의 순매출·취소/마감 정의, 반입 범위·지연·삭제 반영을 기존 결과와 대조.
5. 생성 요청 1건이 후보·검증·게시·원질문 해결까지 이어지는지 확인.
6. 데이터 크기 제한, snapshot 백업·복구, 업데이트·롤백 리허설 확인.

패키지 생성·단위 테스트 성공과 고객 운영계 인수는 별개다. 실제 외부 접속·부하·Linux 배포·복구 리허설을 하지 않은 항목은 release 기록에 미검증으로 남긴다.
