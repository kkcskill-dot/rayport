# Rayport 1.0 APIC 어댑터 통합 계약

이 문서는 **레이포트가 구현한 사용자 정의 HTTP 계약**이다. 현재 고객 APIC의 네이티브 API 스펙이나 연결 검증 결과가 아니다. APIC 측에 이 계약을 충족하는 어댑터와 검증된 업무 API 매핑이 필요하다. 미설정 시 원천 실행은 비활성이고 마트·요청·후보 관리만 사용할 수 있다.

## 설정과 책임

관리자 설정의 `apic`은 `execute_url`, `cancel_url`, `status_url`, `token_env`, `timeout_seconds`(1~30), `contract_version`(`rayport-apic-v1`)을 갖는다. URL은 HTTPS만 허용하며 redirect를 따르지 않는다. 자격증명은 지정한 환경변수에서만 읽고 프론트·로그·계약 export에 포함하지 않는다.

`source_ref`는 원격의 승인된 API 매핑 ID다. `rayport_tool_version`은 **레이포트의 로컬 계약 버전**이며 APIC 네이티브 API 버전과 같다고 가정하면 안 된다. 원격 매핑은 실제 승인 SQL과 네이티브 API 버전을 고정하고, 아래 fingerprint와 계약 hash를 실행 전에 확인해야 한다.

관리자가 `GET /api/v1/tools/{id}/contract`로 내려받는 `apic_binding`에는 `material`, `contract_hash`, `approved_sql_fingerprint`가 있다. 일반 사용자에게 binding을 공개하지 않는다. material은 SQL 원문을 포함하지 않고 SQL fingerprint, 도구/원천 ID, 로컬 버전, 범위·행·신선도 한도, 입력 이름·통화를 담는다. SHA256은 키를 사전순으로 정렬한 JSON의 UTF-8 바이트(공백 없음)에 적용한다. 동일 인코딩의 원문 material도 함께 보관해 다른 언어 구현의 차이를 검증한다.

## 실행

`execute_url`에 POST하고 설정된 서버 자격증명을 Bearer로 전달한다.

```json
{
  "contract_version": "rayport-apic-v1",
  "execution_id": "execution-<unique-id>",
  "tool_id": "<approved-source-ref>",
  "rayport_tool_version": 1,
  "contract_hash": "<approved-binding-sha256>",
  "approved_sql_fingerprint": "<normalized-sql-sha256>",
  "require_contract_match": true,
  "principal_id": "<verified-user-id>",
  "tenant_id": "<configured-tenant>",
  "authorized_store_ids": ["S01"],
  "parameters": {
    "start_date": "2026-11-07",
    "end_date_exclusive": "2026-11-08",
    "store_id": "S01"
  },
  "timeout_seconds": 10
}
```

원격은 사용자 입력 SQL을 받지 않는다. 고정된 매핑의 hash·신원·점포·기간·입력 형식을 검증한 후 바인드 값으로 실행한다. 실행 ID는 중복 실행 방지와 취소/상태 조회에 사용한다. 1.0 APIC 조회는 한 번에 점포 하나이며, 여러 API 매핑도 공동 원천 예산을 사용한다. 24시간 사용자별 APIC 조회 일수 총량 31일도 적용한다. 마트 조회에는 별도 기간 계약이 적용된다.

성공 응답은 다음 envelope다. `result`의 source는 `apic`, dataset_version은 양의 정수, 시각은 RFC3339, 금액은 KRW 원 정수다.

```json
{
  "contract_version": "rayport-apic-v1",
  "execution_id": "execution-<same-id>",
  "contract_hash": "<same-approved-binding-sha256>",
  "approved_sql_fingerprint": "<same-normalized-sql-sha256>",
  "terminated": true,
  "result": {
    "rows": [{"business_date":"2026-11-07","store_id":"S01","gross_minor":120000,"cancel_minor":20000,"currency":"KRW"}],
    "net_minor":100000,"gross_minor":120000,"cancel_minor":20000,
    "currency":"KRW","dataset_version":1,
    "data_as_of":"2026-11-08T06:00:00+09:00","source":"apic"
  }
}
```

레이포트는 hash·실행 ID·종료 여부·범위·행 수·금액·중복·신선도를 재검사한다. 요청 범위 밖 행이 하나라도 있으면 응답 전체를 폐기한다. 반환 행 수 제한이 원천 스캔 제한을 대신하지는 않으므로 APIC/DB 측의 실제 실행 제한과 취소는 별도 필수다.

## 취소와 종료 확인

`cancel_url`, `status_url`에는 같은 본문을 POST한다.

```json
{"contract_version":"rayport-apic-v1","execution_id":"execution-<id>"}
```

실제 DB 실행이 종료되었을 때만 다음을 반환한다.

```json
{"contract_version":"rayport-apic-v1","execution_id":"execution-<id>","terminated":true}
```

HTTP timeout·오류·형식 오류를 DB 종료 증거로 간주하지 않는다. 자동 취소 요청 후 종료를 확인하지 못하면 `in_doubt`로 보관하고 슬롯을 유지한다. 관리자의 종료 확인 동작도 원천의 상태 응답을 확인해야 슬롯을 해제한다. HTTP 요청이 중단되거나 레이포트가 재시작되어도 running APIC 작업을 자동 완료 처리하지 않는다.

## 원격 인수 체크

1. 다른 API/버전/SQL의 hash로 실행을 요청했을 때 **DB 실행 전에** 거절되는지 확인한다. hash를 그대로 echo하는 구현은 검증이 아니다.
2. 신원·조직·점포 권한이 원천 경계에서도 강제되고, 계약에 없는 파라미터를 SQL 조각으로 사용하지 않는지 확인한다.
3. 긴 실행의 timeout·취소·상태 확인과 중복 실행 ID 재전송을 시험한다.
4. 위조된 행·합계·데이터 시각·불완전한 날짜/점포가 결과로 제공되지 않는지 대조한다.
5. 실사용 같은 범위·신선도에서 원천 읽기·CPU·DB time·동시성을 관측한다.

이번 로컬 검증은 fake executor와 in-process HTTP 라우터로 정책을 검사했다. 실제 APIC TLS·취소·DB 부하 검증을 대신하지 않는다.
