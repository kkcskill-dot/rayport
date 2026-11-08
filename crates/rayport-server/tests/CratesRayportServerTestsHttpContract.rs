use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use rayport_adapters::SnapshotStore;
use rayport_application::*;
use rayport_domain::*;
use rayport_server::{
    apic::{DisabledExecutor, SourceError, SourceExecutor, SourceFuture},
    config::{initialize, token_hash, Config, User},
    http::{router, AppState},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tower::ServiceExt;
const ADMIN: &str = "synthetic-admin-test-token-000000000001";
const USER: &str = "synthetic-viewer-test-token-00000000001";
const OTHER: &str = "synthetic-other-test-token-000000000001";
const SQL:&str="SELECT business_date, store_id, gross_minor, cancel_minor, currency FROM store_sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id";
fn principal(id: &str, role: Role) -> Principal {
    Principal {
        id: id.into(),
        tenant_id: "local".into(),
        role,
        store_ids: vec!["S01".into()],
    }
}
fn config() -> Config {
    Config {
        bind: "127.0.0.1:8080".into(),
        public_origin: "http://127.0.0.1:8080".into(),
        secure_cookie: false,
        data_directory: "unused-data".into(),
        web_directory: "unused-web".into(),
        tenant_id: "local".into(),
        users: vec![
            User {
                principal: principal("admin", Role::Admin),
                token_sha256: token_hash(ADMIN),
            },
            User {
                principal: principal("viewer", Role::Viewer),
                token_sha256: token_hash(USER),
            },
            User {
                principal: principal("other", Role::Viewer),
                token_sha256: token_hash(OTHER),
            },
        ],
        apic: None,
    }
}
fn fixture() -> (Router, AppState) {
    fixture_executor(Arc::new(DisabledExecutor))
}
fn fixture_executor(executor: Arc<dyn SourceExecutor>) -> (Router, AppState) {
    let state = AppState::new(
        config(),
        SnapshotStore::memory(Workspace::new("local".into())),
        executor,
    );
    (router(state.clone()), state)
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-rayport-request", "1");
    if let Some(token) = token {
        req = req.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 5 * 1024 * 1024)
        .await
        .unwrap();
    let parsed = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)}));
    (status, parsed)
}
fn tool(source: &str) -> Value {
    json!({"id":"sales.daily","name":"점포 매출","description":"점포별 일매출","keywords":["매출"],"source":source,"source_ref":"daily","sql_template":if source=="apic"{Some(SQL)}else{None},"status":"draft","version":1,"max_days":31,"max_rows":1000,"freshness_seconds":86400,"owner":"sales-owner","validation_evidence":null})
}
fn mart() -> Value {
    json!({"id":"daily","name":"일매출","data_as_of":Utc::now().to_rfc3339(),"complete_through":"2026-11-07","rows":[{"business_date":"2026-11-07","store_id":"S01","gross_minor":120000,"cancel_minor":20000,"currency":"KRW"},{"business_date":"2026-11-07","store_id":"S02","gross_minor":80000,"cancel_minor":5000,"currency":"KRW"}]})
}
fn query() -> Value {
    json!({"start_date":"2026-11-07","end_date_exclusive":"2026-11-08","store_ids":["S01"]})
}
async fn publish(app: &Router, source: &str) {
    if source == "mart" {
        assert_eq!(
            call(app, "POST", "/api/v1/marts/import", Some(ADMIN), mart())
                .await
                .0,
            StatusCode::OK
        );
    }
    let (s, b) = call(app, "POST", "/api/v1/tools", Some(ADMIN), tool(source)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = call(
        app,
        "POST",
        "/api/v1/tools/sales.daily/validate",
        Some(ADMIN),
        json!({"evidence":"합성 정답과 권한 범위 및 취소 계약 검증을 완료함"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, b) = call(
        app,
        "POST",
        "/api/v1/tools/sales.daily/publish",
        Some(ADMIN),
        json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn complete_mart_question_request_lifecycle_without_socket_or_database() {
    let (app, state) = fixture();
    let (_, missing) = call(
        &app,
        "POST",
        "/api/v1/questions/resolve",
        Some(USER),
        json!({"question":"점포 매출"}),
    )
    .await;
    assert_eq!(missing["data"]["status"], "missing");
    call(
        &app,
        "POST",
        "/api/v1/requests",
        Some(USER),
        json!({"question":"점포 매출","purpose":"기능 생성"}),
    )
    .await;
    publish(&app, "mart").await;
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/questions/resolve",
        Some(USER),
        json!({"question":"어제 점포 매출"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["data"]["status"], "matched");
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        Some(USER),
        query(),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["data"]["net_minor"], 100000);
    assert_eq!(b["data"]["rows"].as_array().unwrap().len(), 1);
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/questions/resolve",
        Some(USER),
        json!({"question":"상품군별 매출"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["data"]["status"], "missing");
    let r = json!({"question":"점포 매출","purpose":"기능 생성"});
    let (_, b) = call(&app, "POST", "/api/v1/requests", Some(USER), r.clone()).await;
    let id = b["data"]["id"].as_str().unwrap();
    let (_, again) = call(&app, "POST", "/api/v1/requests", Some(USER), r).await;
    assert_eq!(again["data"]["id"], id);
    let path = format!("/api/v1/requests/{id}/transition");
    for status in ["in_review", "building", "ready"] {
        let (s, b) = call(
            &app,
            "POST",
            &path,
            Some(ADMIN),
            json!({"status":status,"linked_tool_id":"sales.daily"}),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
    }
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(OTHER),
            json!({"status":"resolved"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(USER),
            json!({"status":"resolved"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/tools/sales.daily/query",
            Some(USER),
            query()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &path,
            Some(USER),
            json!({"status":"resolved"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(state.store.read().unwrap().requests[0].status, "resolved");
}
#[tokio::test]
async fn authentication_and_csrf_are_required() {
    let (app, _) = fixture();
    assert_eq!(
        call(&app, "GET", "/api/v1/workspace", None, json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(call(&app,"PUT","/api/v1/policy",Some(USER),json!({"paused":true,"per_user_per_minute":10,"source_per_minute":20,"max_concurrent":2})).await.0,StatusCode::FORBIDDEN);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header("content-type", "application/json")
        .body(Body::from(json!({"token":ADMIN}).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header("content-type", "application/json")
        .header("x-rayport-request", "1")
        .header("origin", "https://untrusted.example")
        .body(Body::from(json!({"token":ADMIN}).to_string()))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}
#[tokio::test]
async fn cookie_login_logout_and_headers() {
    let (app, _) = fixture();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header("content-type", "application/json")
        .header("x-rayport-request", "1")
        .body(Body::from(json!({"token":USER}).to_string()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), 200);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert_eq!(response.headers()["cache-control"], "no-store");
    let pair = cookie.split(';').next().unwrap();
    let req = Request::builder()
        .uri("/api/v1/session")
        .header("cookie", pair)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), 200);
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/session")
        .header("cookie", pair)
        .header("x-rayport-request", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), 200);
    let req = Request::builder()
        .uri("/api/v1/session")
        .header("cookie", pair)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(req).await.unwrap().status(), 401);
}
#[tokio::test]
async fn scope_and_private_metadata_are_enforced_server_side() {
    let (app, _) = fixture();
    publish(&app, "mart").await;
    let q = json!({"start_date":"2026-11-07","end_date_exclusive":"2026-11-08","store_ids":["S01","S02"]});
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        Some(USER),
        q,
    )
    .await;
    assert_eq!(s, 403);
    assert!(b.get("data").is_none());
    let (_, b) = call(&app, "GET", "/api/v1/workspace", Some(USER), json!({})).await;
    assert!(b["data"]["marts"].as_array().unwrap().is_empty());
    assert!(b["data"]["audit"].as_array().unwrap().is_empty());
    assert!(b["data"]["tools"][0]["sql_template"].is_null());
    assert!(b["data"]["tools"][0]["validation_evidence"].is_null());
}
#[tokio::test]
async fn paused_mart_does_not_fallback_or_execute() {
    let (app, state) = fixture();
    publish(&app, "mart").await;
    assert_eq!(call(&app,"PUT","/api/v1/policy",Some(ADMIN),json!({"paused":true,"per_user_per_minute":10,"source_per_minute":20,"max_concurrent":2})).await.0,200);
    let (s, _) = call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        Some(USER),
        query(),
    )
    .await;
    assert_eq!(s, 429);
    let w = state.store.read().unwrap();
    assert_eq!(w.blocked_executions, 1);
    assert!(w.executions.is_empty());
}
#[tokio::test]
async fn stale_or_incomplete_mart_never_becomes_zero() {
    let (app, state) = fixture();
    publish(&app, "mart").await;
    state
        .store
        .transaction(|w| {
            w.marts[0]
                .versions
                .last_mut()
                .unwrap()
                .rows
                .retain(|r| r.store_id != "S01");
            Ok(())
        })
        .unwrap();
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        Some(USER),
        query(),
    )
    .await;
    assert!(!s.is_success());
    assert!(b.get("data").is_none());
    assert_eq!(state.store.read().unwrap().executions[0].status, "failed");
}
#[tokio::test]
async fn discovery_batch_rolls_back_and_unknown_execution_count_stays_null() {
    let (app, state) = fixture();
    let input = json!({"source_label":"synthetic","diagnostics_authorized":true,"observations":[{"sql_text":SQL,"executions":null,"ash_samples":17,"module":null},{"sql_text":"DELETE FROM private_records","executions":1,"ash_samples":0,"module":null}]});
    assert_eq!(
        call(&app, "POST", "/api/v1/discovery/import", Some(ADMIN), input)
            .await
            .0,
        422
    );
    assert!(state.store.read().unwrap().candidates.is_empty());
    let input = json!({"source_label":"synthetic","diagnostics_authorized":true,"observations":[{"sql_text":SQL,"executions":null,"ash_samples":17,"module":null}]});
    let (s, b) = call(&app, "POST", "/api/v1/discovery/import", Some(ADMIN), input).await;
    assert_eq!(s, 200, "{b}");
    assert!(b["data"][0]["executions"].is_null());
    assert_eq!(b["data"][0]["ash_samples"], 17);
}
#[tokio::test]
async fn edited_tool_requires_revalidation_and_optimistic_version() {
    let (app, _) = fixture();
    call(&app, "POST", "/api/v1/marts/import", Some(ADMIN), mart()).await;
    call(&app, "POST", "/api/v1/tools", Some(ADMIN), tool("mart")).await;
    let (s, b) = call(
        &app,
        "PUT",
        "/api/v1/tools/sales.daily",
        Some(ADMIN),
        tool("mart"),
    )
    .await;
    assert_eq!(s, 200, "{b}");
    assert_eq!(b["data"]["version"], 2);
    assert_eq!(b["data"]["status"], "draft");
    assert_eq!(
        call(
            &app,
            "PUT",
            "/api/v1/tools/sales.daily",
            Some(ADMIN),
            tool("mart")
        )
        .await
        .0,
        409
    );
    assert!(!call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/publish",
        Some(ADMIN),
        json!({})
    )
    .await
    .0
    .is_success());
}
struct UncertainExecutor {
    terminated: AtomicBool,
}
impl SourceExecutor for UncertainExecutor {
    fn enabled(&self) -> bool {
        true
    }
    fn execute<'a>(
        &'a self,
        _: &'a str,
        _: &'a Principal,
        _: &'a Tool,
        _: &'a QueryInput,
        _: &'a [String],
    ) -> SourceFuture<'a, Result<QueryResult, SourceError>> {
        Box::pin(async { Err(SourceError::Uncertain) })
    }
    fn reconcile<'a>(&'a self, _: &'a str) -> SourceFuture<'a, bool> {
        Box::pin(async { self.terminated.load(Ordering::SeqCst) })
    }
}
#[tokio::test]
async fn uncertain_source_execution_holds_budget_until_confirmed() {
    let executor = Arc::new(UncertainExecutor {
        terminated: AtomicBool::new(false),
    });
    let (app, state) = fixture_executor(executor.clone());
    publish(&app, "apic").await;
    state
        .store
        .transaction(|w| {
            w.policy.max_concurrent = 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/tools/sales.daily/query",
            Some(USER),
            query()
        )
        .await
        .0,
        502
    );
    let execution = state.store.read().unwrap().executions[0].clone();
    assert_eq!(execution.status, "in_doubt");
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/tools/sales.daily/query",
            Some(USER),
            query()
        )
        .await
        .0,
        429
    );
    let path = format!("/api/v1/executions/{}/reconcile", execution.id);
    assert_eq!(
        call(&app, "POST", &path, Some(ADMIN), json!({})).await.0,
        409
    );
    executor.terminated.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&app, "POST", &path, Some(ADMIN), json!({})).await.0,
        200
    );
    assert_eq!(state.store.read().unwrap().executions[0].status, "failed");
}
struct InvalidScopeExecutor;
impl SourceExecutor for InvalidScopeExecutor {
    fn enabled(&self) -> bool {
        true
    }
    fn execute<'a>(
        &'a self,
        _: &'a str,
        _: &'a Principal,
        _: &'a Tool,
        _: &'a QueryInput,
        _: &'a [String],
    ) -> SourceFuture<'a, Result<QueryResult, SourceError>> {
        Box::pin(async {
            Ok(QueryResult {
                rows: vec![SalesRow {
                    business_date: "2026-11-07".into(),
                    store_id: "S02".into(),
                    gross_minor: 999,
                    cancel_minor: 0,
                    currency: "KRW".into(),
                }],
                net_minor: 999,
                gross_minor: 999,
                cancel_minor: 0,
                currency: "KRW".into(),
                dataset_version: 1,
                data_as_of: Utc::now().to_rfc3339(),
                source: SourceKind::Apic,
            })
        })
    }
    fn reconcile<'a>(&'a self, _: &'a str) -> SourceFuture<'a, bool> {
        Box::pin(async { true })
    }
}
#[tokio::test]
async fn invalid_source_scope_is_discarded_without_data_leak() {
    let (app, _) = fixture_executor(Arc::new(InvalidScopeExecutor));
    publish(&app, "apic").await;
    let (s, b) = call(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        Some(USER),
        query(),
    )
    .await;
    assert_eq!(s, 502);
    assert!(b.get("data").is_none());
    assert!(!b.to_string().contains("S02"));
}
#[tokio::test]
async fn disabled_apic_cannot_be_published() {
    let (app, _) = fixture();
    assert_eq!(
        call(&app, "POST", "/api/v1/tools", Some(ADMIN), tool("apic"))
            .await
            .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/tools/sales.daily/validate",
            Some(ADMIN),
            json!({"evidence":"범위와 신원 검증의 합성 테스트 근거입니다"})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/tools/sales.daily/publish",
            Some(ADMIN),
            json!({})
        )
        .await
        .0,
        409
    );
}
#[test]
fn initialization_is_private_idempotent_and_does_not_start_services() {
    let dir = tempfile::tempdir().unwrap();
    initialize(dir.path()).unwrap();
    let c = Config::load(&dir.path().join("config.json")).unwrap();
    assert_eq!(c.users.len(), 1);
    assert!(c.users[0].principal.store_ids.is_empty());
    assert!(c.apic.is_none());
    let before = std::fs::read(dir.path().join("config.json")).unwrap();
    assert!(initialize(dir.path()).is_err());
    assert_eq!(
        before,
        std::fs::read(dir.path().join("config.json")).unwrap()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.path().join("initial-admin-token.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn plaintext_public_binding_is_rejected_and_https_origin_requires_secure_cookie() {
    let mut c = config();
    c.bind = "0.0.0.0:8080".into();
    assert!(c.validate().is_err());
    c.public_origin = "https://rayport.example".into();
    assert!(c.validate().is_err());
    c.secure_cookie = true;
    assert!(c.validate().is_ok());
}
#[test]
fn source_binding_changes_for_mapping_policy_version_and_sql() {
    use rayport_server::apic::approved_binding;
    let mut t: Tool = serde_json::from_value(tool("apic")).unwrap();
    let original = approved_binding(&t).unwrap();
    t.max_days += 1;
    assert_ne!(
        original.contract_hash,
        approved_binding(&t).unwrap().contract_hash
    );
    t = serde_json::from_value(tool("apic")).unwrap();
    t.source_ref = "different-api".into();
    assert_ne!(
        original.contract_hash,
        approved_binding(&t).unwrap().contract_hash
    );
    t.version += 1;
    let hash = approved_binding(&t).unwrap().contract_hash;
    t.sql_template = Some(SQL.replace("store_sales", "different_sales"));
    assert_ne!(hash, approved_binding(&t).unwrap().contract_hash);
    t.sql_template = Some("DELETE FROM records".into());
    assert!(approved_binding(&t).is_err());
}

#[test]
fn noncanonical_origins_are_rejected_before_startup() {
    let mut c = config();
    c.secure_cookie = true;
    for origin in [
        "https://rayport.example/",
        "https://RAYPORT.example",
        "https://rayport.example:443",
    ] {
        c.public_origin = origin.into();
        assert!(c.validate().is_err(), "{origin}");
    }
    c.public_origin = "https://rayport.example".into();
    assert!(c.validate().is_ok());
}
