use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use rayport_adapters::SnapshotStore;
use rayport_application::Workspace;
use rayport_domain::{Principal, QueryInput, QueryResult, Role, SourceKind, Tool, ToolStatus};
use rayport_server::{
    apic::{DisabledExecutor, SourceError, SourceExecutor, SourceFuture},
    config::{token_hash, Config, User},
    http::{router, AppState},
};
use serde_json::{json, Value};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tower::ServiceExt;

const ADMIN: &str = "synthetic-http-hardening-admin-token-0001";
const VIEWER: &str = "synthetic-http-hardening-viewer-token-0001";
const SQL: &str = "SELECT business_date, store_id, gross_minor, cancel_minor, currency FROM store_sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id";

fn principal(id: &str, role: Role) -> Principal {
    Principal {
        id: id.into(),
        tenant_id: "local".into(),
        role,
        store_ids: vec!["S01".into()],
    }
}

fn fixture(executor: Arc<dyn SourceExecutor>) -> (Router, AppState) {
    let config = Config {
        bind: "127.0.0.1:8080".into(),
        public_origin: "http://127.0.0.1:8080".into(),
        secure_cookie: false,
        data_directory: "unused-data".into(),
        web_directory: "unused-web".into(),
        tenant_id: "local".into(),
        apic: None,
        users: vec![
            User {
                principal: principal("admin", Role::Admin),
                token_sha256: token_hash(ADMIN),
            },
            User {
                principal: principal("viewer", Role::Viewer),
                token_sha256: token_hash(VIEWER),
            },
        ],
    };
    let state = AppState::new(
        config,
        SnapshotStore::memory(Workspace::new("local".into())),
        executor,
    );
    (router(state.clone()), state)
}

fn tool(status: ToolStatus, source: SourceKind) -> Tool {
    Tool {
        id: "sales.daily".into(),
        name: "점포 매출".into(),
        description: "승인된 점포별 매출 조회".into(),
        keywords: vec!["매출".into()],
        source,
        source_ref: "private-remote-tool".into(),
        sql_template: Some(SQL.into()),
        status,
        version: 7,
        max_days: 17,
        max_rows: 321,
        freshness_seconds: 7200,
        owner: "owner".into(),
        validation_evidence: Some("internal-evidence-never-export".into()),
    }
}

async fn raw_request(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    content_type: &str,
    body: String,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("x-rayport-request", "1")
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let bytes = to_bytes(response.into_body(), 5 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn contract(app: &Router, token: &str) -> (StatusCode, Value) {
    raw_request(
        app,
        "GET",
        "/api/v1/tools/sales.daily/contract",
        token,
        "application/json",
        String::new(),
    )
    .await
}

#[tokio::test]
async fn malformed_unknown_and_wrong_type_json_are_safe_error_envelopes() {
    let (app, _) = fixture(Arc::new(DisabledExecutor));
    let cases = [
        ("{\"question\":", StatusCode::BAD_REQUEST),
        (
            "{\"question\":\"매출\",\"never-echo-this-field\":\"private-input\"}",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "{\"question\":{\"private-value\":true}}",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ];
    for (body, expected) in cases {
        let (status, value) = raw_request(
            &app,
            "POST",
            "/api/v1/questions/resolve",
            VIEWER,
            "application/json",
            body.into(),
        )
        .await;
        assert_eq!(status, expected);
        assert_eq!(value["error"]["code"], "invalid_json");
        assert!(value.get("data").is_none());
        let error = value.to_string();
        assert!(!error.contains("never-echo"));
        assert!(!error.contains("private-"));
        assert!(!error.contains("expected"));
        assert!(!error.contains("line"));
    }
}

#[tokio::test]
async fn oversized_and_non_json_requests_keep_status_and_envelope() {
    let (app, _) = fixture(Arc::new(DisabledExecutor));
    let (status, value) = raw_request(
        &app,
        "POST",
        "/api/v1/questions/resolve",
        VIEWER,
        "text/plain",
        "private-request".into(),
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(value["error"]["code"], "unsupported_media_type");
    let large = json!({"question": "x".repeat(4 * 1024 * 1024)}).to_string();
    let (status, value) = raw_request(
        &app,
        "POST",
        "/api/v1/questions/resolve",
        VIEWER,
        "application/json",
        large,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(value["error"]["code"], "payload_too_large");
    assert!(value.get("data").is_none());
}

#[tokio::test]
async fn every_json_endpoint_uses_the_custom_rejection_shape() {
    let (app, _) = fixture(Arc::new(DisabledExecutor));
    for (method, path) in [
        ("POST", "/api/v1/session"),
        ("POST", "/api/v1/tools"),
        ("PUT", "/api/v1/tools/sales.daily"),
        ("POST", "/api/v1/tools/sales.daily/validate"),
        ("POST", "/api/v1/tools/sales.daily/query"),
        ("POST", "/api/v1/requests"),
        ("POST", "/api/v1/requests/request-id/transition"),
        ("POST", "/api/v1/discovery/import"),
        ("POST", "/api/v1/candidates/candidate-id/draft"),
        ("POST", "/api/v1/marts/import"),
        ("PUT", "/api/v1/policy"),
    ] {
        let (status, value) =
            raw_request(&app, method, path, ADMIN, "application/json", "[".into()).await;
        assert!(matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
        assert_eq!(value["error"]["code"], "invalid_json", "{path}");
    }
}

#[tokio::test]
async fn draft_contract_is_admin_only_and_export_contains_no_raw_sql_or_evidence() {
    let (app, state) = fixture(Arc::new(DisabledExecutor));
    state
        .store
        .transaction(|workspace| {
            workspace
                .tools
                .push(tool(ToolStatus::Draft, SourceKind::Apic));
            Ok(())
        })
        .unwrap();
    assert_eq!(contract(&app, VIEWER).await.0, StatusCode::NOT_FOUND);
    let (status, value) = contract(&app, ADMIN).await;
    assert_eq!(status, StatusCode::OK);
    let definition = &value["data"];
    assert_eq!(definition["publication_status"], "draft");
    assert_eq!(definition["tool_id"], "sales.daily");
    assert_eq!(definition["version"], 7);
    assert_eq!(definition["metadata_only"], true);
    assert_eq!(definition["mcp_transport_registered"], false);
    assert_eq!(
        definition["apic_binding"]["material"]["tool_id"],
        "private-remote-tool"
    );
    assert_eq!(
        definition["apic_binding"]["contract_hash"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert!(!value.to_string().contains(SQL));
    assert!(!value.to_string().contains("internal-evidence"));
    assert!(!value.to_string().contains("sql_template"));
}

#[tokio::test]
async fn published_export_has_matching_versions_schemas_auth_and_live_limits() {
    let (app, state) = fixture(Arc::new(DisabledExecutor));
    state
        .store
        .transaction(|workspace| {
            workspace
                .tools
                .push(tool(ToolStatus::Published, SourceKind::Apic));
            Ok(())
        })
        .unwrap();
    let (status, value) = contract(&app, VIEWER).await;
    assert_eq!(status, StatusCode::OK);
    let definition = &value["data"];
    assert!(definition["apic_binding"].is_null());
    assert!(!value.to_string().contains("private-remote-tool"));
    assert_eq!(definition["openapi"]["openapi"], "3.1.1");
    assert_eq!(definition["openapi"]["info"]["version"], "7");
    let operation = &definition["openapi"]["paths"]["/api/v1/tools/sales.daily/query"]["post"];
    assert_eq!(operation["x-rayport-tool-version"], definition["version"]);
    assert_eq!(operation["x-rayport-max-days"], 17);
    assert_eq!(operation["x-rayport-max-rows"], 321);
    assert_eq!(operation["x-rayport-freshness-seconds"], 7200);
    assert_eq!(operation["parameters"][0]["name"], "X-Rayport-Request");
    assert_eq!(operation["parameters"][0]["required"], true);
    assert_eq!(operation["security"].as_array().unwrap().len(), 2);
    assert_eq!(definition["mcp_tool"]["annotations"]["readOnlyHint"], true);
    assert_eq!(
        definition["mcp_tool"]["inputSchema"],
        definition["openapi"]["components"]["schemas"]["QueryInput"]
    );
    assert_eq!(
        definition["mcp_tool"]["inputSchema"],
        operation["requestBody"]["content"]["application/json"]["schema"]
    );
    assert_eq!(
        definition["openapi"]["components"]["schemas"]["QueryResult"]["properties"]["rows"]
            ["maxItems"],
        321
    );
    state
        .store
        .transaction(|workspace| {
            workspace.tools[0].version = 8;
            workspace.tools[0].max_days = 5;
            Ok(())
        })
        .unwrap();
    let (_, updated) = contract(&app, VIEWER).await;
    assert_eq!(updated["data"]["version"], 8);
    assert_eq!(
        updated["data"]["mcp_tool"]["inputSchema"]["x-rayport-max-days"],
        5
    );
}

#[tokio::test]
async fn contract_export_enforces_tenant_boundary() {
    let (app, state) = fixture(Arc::new(DisabledExecutor));
    state
        .store
        .transaction(|workspace| {
            workspace.tenant_id = "other-tenant".into();
            workspace
                .tools
                .push(tool(ToolStatus::Published, SourceKind::Mart));
            Ok(())
        })
        .unwrap();
    assert_eq!(contract(&app, ADMIN).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "current_thread")]
async fn blocked_snapshot_read_does_not_block_the_async_runtime() {
    let (app, state) = fixture(Arc::new(DisabledExecutor));
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let store = state.store.clone();
    let lock_thread = std::thread::spawn(move || {
        store
            .transaction(|_| {
                entered_sender.send(()).unwrap();
                let _ = release_receiver.recv_timeout(Duration::from_secs(2));
                Ok(())
            })
            .unwrap();
    });
    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    let request = Request::builder()
        .uri("/api/v1/workspace")
        .header("authorization", format!("Bearer {VIEWER}"))
        .body(Body::empty())
        .unwrap();
    let started = Instant::now();
    let pending = tokio::spawn(app.clone().oneshot(request));
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "storage blocked the runtime"
    );
    release_sender.send(()).unwrap();
    lock_thread.join().unwrap();
    assert_eq!(pending.await.unwrap().unwrap().status(), StatusCode::OK);
}

struct PendingExecutor {
    started: tokio::sync::Notify,
}
impl SourceExecutor for PendingExecutor {
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
        Box::pin(async move {
            self.started.notify_one();
            std::future::pending().await
        })
    }
    fn reconcile<'a>(&'a self, _: &'a str) -> SourceFuture<'a, bool> {
        Box::pin(async { false })
    }
}

#[tokio::test]
async fn dropping_http_future_keeps_reserved_source_capacity_fail_closed() {
    let executor = Arc::new(PendingExecutor {
        started: tokio::sync::Notify::new(),
    });
    let (app, state) = fixture(executor.clone());
    state
        .store
        .transaction(|workspace| {
            workspace
                .tools
                .push(tool(ToolStatus::Published, SourceKind::Apic));
            workspace.policy.max_concurrent = 1;
            Ok(())
        })
        .unwrap();
    let body =
        json!({"start_date":"2026-11-07","end_date_exclusive":"2026-11-08","store_ids":["S01"]});
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/tools/sales.daily/query")
        .header("content-type", "application/json")
        .header("x-rayport-request", "1")
        .header("authorization", format!("Bearer {VIEWER}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let pending = tokio::spawn(app.clone().oneshot(request));
    tokio::time::timeout(Duration::from_secs(2), executor.started.notified())
        .await
        .unwrap();
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(state.store.read().unwrap().executions[0].status, "running");
    let (status, value) = raw_request(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        VIEWER,
        "application/json",
        body.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{value}");
    assert_eq!(state.store.read().unwrap().blocked_executions, 1);
}

#[tokio::test]
async fn canceled_mart_http_future_finishes_its_local_reservation() {
    let (app, state) = fixture(Arc::new(DisabledExecutor));
    state
        .store
        .transaction(|w| {
            let mut t = tool(ToolStatus::Published, SourceKind::Mart);
            t.source_ref = "daily".into();
            w.tools.push(t);
            w.policy.max_concurrent = 1;
            w.import_mart(
                &principal("admin", Role::Admin),
                rayport_application::MartImport {
                    id: "daily".into(),
                    name: "합성 일매출".into(),
                    data_as_of: chrono::Utc::now().to_rfc3339(),
                    complete_through: "2026-11-07".into(),
                    rows: vec![rayport_domain::SalesRow {
                        business_date: "2026-11-07".into(),
                        store_id: "S01".into(),
                        gross_minor: 100,
                        cancel_minor: 0,
                        currency: "KRW".into(),
                    }],
                },
                chrono::Utc::now(),
            )
            .unwrap();
            Ok(())
        })
        .unwrap();
    let store = state.store.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let lock_thread = std::thread::spawn(move || {
        store
            .transaction(|_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
                Ok(())
            })
            .unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let body =
        json!({"start_date":"2026-11-07","end_date_exclusive":"2026-11-08","store_ids":["S01"]});
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/tools/sales.daily/query")
        .header("content-type", "application/json")
        .header("x-rayport-request", "1")
        .header("authorization", format!("Bearer {VIEWER}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let pending = tokio::spawn(app.clone().oneshot(request));
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    release_tx.send(()).unwrap();
    lock_thread.join().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = state.store.read().unwrap();
            if let Some(e) = snapshot.executions.first() {
                assert_eq!(e.status, "completed");
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (status, value) = raw_request(
        &app,
        "POST",
        "/api/v1/tools/sales.daily/query",
        VIEWER,
        "application/json",
        body.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
