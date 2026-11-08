use crate::{
    apic::{SourceError, SourceExecutor},
    config::{token_hash, Config},
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, DefaultBodyLimit, FromRequest, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use rayport_adapters::{SnapshotStore, StoreError};
use rayport_application::*;
use rayport_domain::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Arc<SnapshotStore<Workspace>>,
    pub executor: Arc<dyn SourceExecutor>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    login_attempts: Arc<Mutex<Vec<i64>>>,
    storage_slots: Arc<Semaphore>,
}
#[derive(Clone)]
struct Session {
    principal: Principal,
    expires: i64,
}
impl AppState {
    pub fn new(
        config: Config,
        store: SnapshotStore<Workspace>,
        executor: Arc<dyn SourceExecutor>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            store: Arc::new(store),
            executor,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            login_attempts: Arc::new(Mutex::new(vec![])),
            storage_slots: Arc::new(Semaphore::new(4)),
        }
    }
    async fn blocking<R: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<SnapshotStore<Workspace>>) -> Result<R, ApiError> + Send + 'static,
    ) -> Result<R, ApiError> {
        let permit = self
            .storage_slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "storage_unavailable",
                    "저장 작업을 처리할 수 없습니다.",
                )
            })?;
        let store = self.store.clone();
        // The worker owns its permit. Dropping an HTTP future cannot release
        // capacity before a running fsync/transaction actually completes.
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation(store)
        })
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "저장 작업이 중단되었습니다. 운영자에게 확인하세요.",
            )
        })?
    }
    async fn read<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&Workspace) -> AppResult<R> + Send + 'static,
    ) -> Result<R, ApiError> {
        self.blocking(move |store| {
            let snapshot = store.read().map_err(ApiError::storage)?;
            operation(&snapshot).map_err(Into::into)
        })
        .await
    }
    async fn write<R: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Workspace) -> AppResult<R> + Send + 'static,
    ) -> Result<R, ApiError> {
        self.blocking(move |store| {
            store
                .transaction(|workspace| {
                    operation(workspace).map_err(|error| {
                        StoreError::Rejected(serde_json::to_string(&error).unwrap_or_default())
                    })
                })
                .map_err(ApiError::storage)
        })
        .await
    }
}
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, code: &str, message: &str) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    fn storage(e: StoreError) -> Self {
        match e {
            StoreError::Rejected(s) => serde_json::from_str::<DomainError>(&s)
                .map(Into::into)
                .unwrap_or_else(|_| {
                    Self::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid_request",
                        "입력 내용을 확인하세요.",
                    )
                }),
            StoreError::Unavailable(_) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "저장소를 사용할 수 없습니다. 운영자에게 확인하세요.",
            ),
        }
    }
}
impl From<DomainError> for ApiError {
    fn from(e: DomainError) -> Self {
        let status = match e.code.as_str() {
            "forbidden" | "unauthorized_store" | "scope_denied" => StatusCode::FORBIDDEN,
            "not_found" => StatusCode::NOT_FOUND,
            "conflict" | "invalid_transition" | "source_disabled" | "tool_retired" => {
                StatusCode::CONFLICT
            }
            "budget_exceeded" | "capacity" => StatusCode::TOO_MANY_REQUESTS,
            "missing_source" | "stale_data" | "incomplete_data" => StatusCode::CONFLICT,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        Self {
            status,
            code: e.code,
            message: e.message,
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
/// All JSON extraction failures use the same public envelope. Never echo
/// parser diagnostics, field names, SQL, tokens, or submitted values.
struct ApiJson<T>(T);
impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;
    async fn from_request(
        request: axum::http::Request<Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|rejection: JsonRejection| {
                let status = rejection.status();
                let (code, message) = match status {
                    StatusCode::PAYLOAD_TOO_LARGE => {
                        ("payload_too_large", "요청 본문이 허용 크기를 초과했습니다.")
                    }
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => (
                        "unsupported_media_type",
                        "application/json 형식으로 요청하세요.",
                    ),
                    _ => ("invalid_json", "요청 JSON의 형식과 필수 항목을 확인하세요."),
                };
                ApiError::new(status, code, message)
            })
    }
}
#[derive(Serialize)]
struct Data<T> {
    data: T,
}
fn data<T: Serialize>(value: T) -> Json<Data<T>> {
    Json(Data { data: value })
}
type ApiResult<T> = Result<Json<Data<T>>, ApiError>;
fn unauthorized() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "로그인이 필요합니다.",
    )
}
fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|s| s.strip_prefix("rayport_session="))
}
fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Principal, ApiError> {
    if let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        return find_token(state, bearer).ok_or_else(unauthorized);
    }
    let token = cookie(headers).ok_or_else(unauthorized)?;
    let hash = token_hash(token);
    let mut sessions = state.sessions.lock().map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "session_unavailable",
            "세션을 확인할 수 없습니다.",
        )
    })?;
    sessions.retain(|_, s| s.expires > Utc::now().timestamp());
    sessions
        .get(&hash)
        .map(|s| s.principal.clone())
        .ok_or_else(unauthorized)
}
fn find_token(state: &AppState, token: &str) -> Option<Principal> {
    if !(32..=256).contains(&token.len()) {
        return None;
    }
    let digest = token_hash(token);
    state
        .config
        .users
        .iter()
        .find(|u| bool::from(digest.as_bytes().ct_eq(u.token_sha256.as_bytes())))
        .map(|u| u.principal.clone())
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route(
            "/session",
            get(session_get).post(session_create).delete(session_delete),
        )
        .route("/workspace", get(workspace))
        .route("/tools", post(tool_create))
        .route("/tools/{id}", put(tool_edit))
        .route("/tools/{id}/validate", post(tool_validate))
        .route("/tools/{id}/publish", post(tool_publish))
        .route("/tools/{id}/retire", post(tool_retire))
        .route("/tools/{id}/query", post(tool_query))
        .route("/tools/{id}/contract", get(tool_contract))
        .route("/questions/resolve", post(question_resolve))
        .route("/requests", post(request_create))
        .route("/requests/{id}/transition", post(request_transition))
        .route("/discovery/import", post(discovery_import))
        .route("/candidates/{id}/draft", post(candidate_draft))
        .route("/marts/import", post(mart_import))
        .route("/policy", put(policy_update))
        .route("/executions", get(executions))
        .route("/executions/{id}/reconcile", post(execution_reconcile))
        .fallback(|| async {
            ApiError::new(StatusCode::NOT_FOUND, "not_found", "API 경로가 없습니다.")
        });
    let web = ServeDir::new(&state.config.web_directory).not_found_service(ServeFile::new(
        state.config.web_directory.join("index.html"),
    ));
    Router::new()
        .route(
            "/healthz",
            get(|| async {
                Json(serde_json::json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}))
            }),
        )
        .route("/readyz", get(ready))
        .nest("/api/v1", api)
        .fallback_service(web)
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), security))
        .with_state(state)
}
async fn security(
    State(state): State<AppState>,
    request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        if request
            .headers()
            .get("x-rayport-request")
            .and_then(|h| h.to_str().ok())
            != Some("1")
        {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "요청 출처 확인 헤더가 필요합니다.",
            )
            .into_response();
        }
        if let Some(origin) = request.headers().get(header::ORIGIN) {
            if origin.to_str().ok() != Some(state.config.public_origin.as_str()) {
                return ApiError::new(
                    StatusCode::FORBIDDEN,
                    "origin_rejected",
                    "허용되지 않는 요청 출처입니다.",
                )
                .into_response();
            }
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert("content-security-policy",HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"));
    response
}
async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.read(|_| Ok(())).await?;
    Ok(Json(
        serde_json::json!({"status":"ready","version":env!("CARGO_PKG_VERSION"),"source_execution_enabled":state.executor.enabled()}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    token: String,
}
async fn session_create(
    State(state): State<AppState>,
    ApiJson(input): ApiJson<Login>,
) -> Result<Response, ApiError> {
    {
        let mut attempts = state.login_attempts.lock().map_err(|_| unauthorized())?;
        let now = Utc::now().timestamp();
        attempts.retain(|t| *t > now - 60);
        if attempts.len() >= 30 {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "login_rate_limited",
                "로그인 시도가 많습니다. 잠시 후 재시도하세요.",
            ));
        }
        attempts.push(now);
    }
    let principal = find_token(&state, &input.token).ok_or_else(unauthorized)?;
    let value = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut sessions = state.sessions.lock().map_err(|_| unauthorized())?;
    sessions.retain(|_, s| s.expires > Utc::now().timestamp());
    if sessions.len() >= 1000 {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "session_limit",
            "세션 한도를 초과했습니다.",
        ));
    }
    sessions.insert(
        token_hash(&value),
        Session {
            principal: principal.clone(),
            expires: Utc::now().timestamp() + 28800,
        },
    );
    let suffix = if state.config.secure_cookie {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!(
        "rayport_session={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800{suffix}"
    );
    let mut response = data(principal).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| unauthorized())?,
    );
    Ok(response)
}
async fn session_get(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Principal> {
    Ok(data(authenticate(&state, &headers)?))
}
async fn session_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(c) = cookie(&headers) {
        state
            .sessions
            .lock()
            .map_err(|_| unauthorized())?
            .remove(&token_hash(c));
    }
    let mut response = data(serde_json::json!({"logged_out":true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("rayport_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    );
    Ok(response)
}
async fn workspace(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<WorkspaceView> {
    let principal = authenticate(&state, &headers)?;
    let enabled = state.executor.enabled();
    Ok(data(
        state
            .read(move |workspace| workspace.view(&principal, enabled))
            .await?,
    ))
}
async fn tool_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(tool): ApiJson<Tool>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| workspace.create_tool(&principal, tool, Utc::now()))
            .await?,
    ))
}
async fn tool_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(tool): ApiJson<Tool>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| workspace.edit_tool(&principal, &id, tool, Utc::now()))
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    evidence: String,
}
async fn tool_validate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<Evidence>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| {
                workspace.validate(&principal, &id, &input.evidence, Utc::now())
            })
            .await?,
    ))
}
async fn tool_publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    let enabled = state.executor.enabled();
    Ok(data(
        state
            .write(move |workspace| {
                workspace.admin(&principal)?;
                if workspace.tool(&id)?.source == SourceKind::Apic && !enabled {
                    return Err(DomainError::new(
                        "source_disabled",
                        "APIC 실행 계약이 설정되지 않았습니다.",
                    ));
                }
                workspace.transition(&principal, &id, ToolStatus::Published, Utc::now())
            })
            .await?,
    ))
}
async fn tool_retire(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| {
                workspace.transition(&principal, &id, ToolStatus::Retired, Utc::now())
            })
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Question {
    question: String,
}
async fn question_resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Question>,
) -> ApiResult<Resolution> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .read(move |workspace| workspace.resolve(&principal, &input.question))
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestInput {
    question: String,
    purpose: String,
}
async fn request_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<RequestInput>,
) -> ApiResult<CapabilityRequest> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| {
                workspace.request(&principal, &input.question, &input.purpose, Utc::now())
            })
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestTransition {
    status: String,
    linked_tool_id: Option<String>,
}
async fn request_transition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<RequestTransition>,
) -> ApiResult<CapabilityRequest> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| {
                workspace.transition_request(
                    &principal,
                    &id,
                    &input.status,
                    input.linked_tool_id,
                    Utc::now(),
                )
            })
            .await?,
    ))
}
async fn discovery_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<DiscoveryImport>,
) -> ApiResult<Vec<Candidate>> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| workspace.import_discovery(&principal, input, Utc::now()))
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftInput {
    name: String,
    source_ref: String,
    source: SourceKind,
    owner: String,
}
async fn candidate_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<DraftInput>,
) -> ApiResult<Tool> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| {
                workspace.candidate_draft(
                    &principal,
                    &id,
                    CandidateDraftCommand {
                        name: input.name,
                        source_ref: input.source_ref,
                        source: input.source,
                        owner: input.owner,
                    },
                    Utc::now(),
                )
            })
            .await?,
    ))
}
async fn mart_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<MartImport>,
) -> ApiResult<MartSummary> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| workspace.import_mart(&principal, input, Utc::now()))
            .await?,
    ))
}
async fn policy_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Policy>,
) -> ApiResult<Policy> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .write(move |workspace| workspace.policy(&principal, input, Utc::now()))
            .await?,
    ))
}
async fn executions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<Execution>> {
    let principal = authenticate(&state, &headers)?;
    Ok(data(
        state
            .read(move |workspace| {
                workspace.admin(&principal)?;
                Ok(workspace
                    .executions
                    .iter()
                    .filter(|execution| matches!(execution.status.as_str(), "running" | "in_doubt"))
                    .cloned()
                    .collect())
            })
            .await?,
    ))
}
async fn execution_reconcile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let principal = authenticate(&state, &headers)?;
    let execution_id = id.clone();
    state
        .read(move |workspace| {
            workspace.admin(&principal)?;
            if !workspace.executions.iter().any(|execution| {
                execution.id == execution_id
                    && execution.source == SourceKind::Apic
                    && matches!(execution.status.as_str(), "running" | "in_doubt")
            }) {
                return Err(DomainError::new(
                    "not_found",
                    "종료 확인 대상 실행이 없습니다.",
                ));
            }
            Ok(())
        })
        .await?;
    if !state.executor.reconcile(&id).await {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "termination_unconfirmed",
            "DB 실행 종료를 확인하지 못했습니다. 실행 슬롯을 유지합니다.",
        ));
    }
    state
        .write(move |workspace| workspace.finish(&id, "failed"))
        .await?;
    Ok(data(serde_json::json!({ "terminated": true })))
}

enum PreparedQuery {
    Mart(AppResult<QueryResult>),
    Source(Box<(Tool, String, Vec<String>)>),
}

async fn tool_query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    ApiJson(input): ApiJson<QueryInput>,
) -> ApiResult<QueryResult> {
    let principal = authenticate(&state, &headers)?;
    let admission_principal = principal.clone();
    let admission_input = input.clone();
    let source_enabled = state.executor.enabled();
    // Admission and durable reservation are one serialized transaction. A
    // canceled HTTP future leaves source reservations fail-closed. Local mart
    // admission, calculation and completion share the same blocking transaction.
    let prepared = state
        .write(move |workspace| {
            let tool = workspace.tool(&id)?;
            workspace.authorize(&admission_principal)?;
            if tool.source == SourceKind::Apic && !source_enabled {
                return Err(DomainError::new(
                    "source_disabled",
                    "APIC 실행이 비활성입니다.",
                ));
            }
            let authorized = authorize_query(&admission_principal, &tool, &admission_input)?;
            if tool.source == SourceKind::Apic && authorized.len() != 1 {
                return Err(DomainError::new(
                    "invalid_scope",
                    "APIC 1.0 조회는 점포 하나를 지정해야 합니다.",
                ));
            }
            match workspace.reserve(&admission_principal, &tool, &admission_input, Utc::now()) {
                Ok((execution_id, scope)) => {
                    if tool.source == SourceKind::Mart {
                        let result =
                            workspace.query_mart(&tool, &admission_input, &scope, Utc::now());
                        workspace.finish(
                            &execution_id,
                            if result.is_ok() {
                                "completed"
                            } else {
                                "failed"
                            },
                        )?;
                        Ok(Ok(PreparedQuery::Mart(result)))
                    } else {
                        Ok(Ok(PreparedQuery::Source(Box::new((
                            tool,
                            execution_id,
                            scope,
                        )))))
                    }
                }
                Err(error) => {
                    workspace.blocked_executions = workspace.blocked_executions.saturating_add(1);
                    Ok(Err(error))
                }
            }
        })
        .await??;
    let (tool, execution_id, scope) = match prepared {
        PreparedQuery::Mart(result) => return Ok(data(result?)),
        PreparedQuery::Source(source) => *source,
    };
    match state
        .executor
        .execute(&execution_id, &principal, &tool, &input, &scope)
        .await
    {
        Ok(result) => {
            let validation = validate_source_result(&result, &tool, &input, &scope);
            let status = if validation.is_ok() {
                "completed"
            } else {
                "failed"
            };
            state
                .write(move |workspace| workspace.finish(&execution_id, status))
                .await?;
            validation?;
            // A tool retired while the remote call was running must not release its result.
            state
                .read(move |workspace| {
                    if workspace.tool(&tool.id)?.status != ToolStatus::Published {
                        return Err(DomainError::new(
                            "tool_retired",
                            "조회 중 도구가 폐기되어 결과를 제공하지 않습니다.",
                        ));
                    }
                    Ok(())
                })
                .await?;
            Ok(data(result))
        }
        Err(error) => {
            let uncertain = matches!(error, SourceError::Uncertain);
            state
                .write(move |workspace| {
                    workspace.finish(&execution_id, if uncertain { "in_doubt" } else { "failed" })
                })
                .await?;
            Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                if uncertain {
                    "execution_in_doubt"
                } else {
                    "source_rejected"
                },
                if uncertain {
                    "원천 실행 종료가 확인되지 않았습니다. 슬롯을 유지하고 자동 재시도하지 않습니다."
                } else {
                    "원천이 조회를 거절하거나 취소했습니다."
                },
            ))
        }
    }
}

async fn tool_contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let principal = authenticate(&state, &headers)?;
    let origin = state.config.public_origin.clone();
    let definition = state
        .read(move |workspace| {
            workspace.authorize(&principal)?;
            let tool = workspace.tool(&id)?;
            if principal.role != Role::Admin && tool.status != ToolStatus::Published {
                return Err(DomainError::new("not_found", "도구를 찾을 수 없습니다."));
            }
            let binding = if principal.role == Role::Admin && tool.source == SourceKind::Apic {
                crate::apic::approved_binding(&tool).ok()
            } else {
                None
            };
            let mut definition = export_contract(&tool, &origin);
            definition["apic_binding"] = serde_json::json!(binding);
            Ok(definition)
        })
        .await?;
    Ok(data(definition))
}

/// A metadata export only. It neither registers a tool nor starts an MCP transport.
fn export_contract(tool: &Tool, origin: &str) -> serde_json::Value {
    use serde_json::json;
    let identifier = json!({
        "type": "string", "minLength": 1, "maxLength": 128,
        "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]*$", "not": {"pattern": "\\.\\."}
    });
    let input_schema = json!({
        "type": "object", "additionalProperties": false,
        "required": ["start_date", "end_date_exclusive"],
        "properties": {
            "start_date": {"type": "string", "format": "date", "description": "조회 시작 영업일, 포함"},
            "end_date_exclusive": {"type": "string", "format": "date", "description": "조회 종료 영업일, 미포함"},
            "store_ids": {"type": "array", "uniqueItems": true, "maxItems": if tool.source == SourceKind::Apic {1} else {1000},
                "items": identifier, "description": "생략하거나 빈 배열이면 서버의 허용 점포 범위를 적용합니다. APIC는 최종 점포 한 개만 허용합니다."}
        },
        "x-rayport-max-days": tool.max_days
    });
    let money = json!({"type": "integer", "minimum": -MAX_SAFE_MINOR, "maximum": MAX_SAFE_MINOR,
        "description": "KRW 정수 원 단위; 부동소수점 금액을 사용하지 않습니다."});
    let nonnegative_money = json!({"type": "integer", "minimum": 0, "maximum": MAX_SAFE_MINOR});
    let row = json!({
        "type": "object", "additionalProperties": false,
        "required": ["business_date", "store_id", "gross_minor", "cancel_minor", "currency"],
        "properties": {
            "business_date": {"type": "string", "format": "date"}, "store_id": identifier,
            "gross_minor": nonnegative_money, "cancel_minor": nonnegative_money,
            "currency": {"type": "string", "const": "KRW"}
        }
    });
    let query_result = json!({
        "type": "object", "additionalProperties": false,
        "required": ["rows", "net_minor", "gross_minor", "cancel_minor", "currency", "dataset_version", "data_as_of", "source"],
        "properties": {
            "rows": {"type": "array", "maxItems": tool.max_rows, "items": row},
            "net_minor": money, "gross_minor": nonnegative_money, "cancel_minor": nonnegative_money,
            "currency": {"type": "string", "const": "KRW"},
            "dataset_version": {"type": "integer", "minimum": 1},
            "data_as_of": {"type": "string", "format": "date-time"},
            "source": {"type": "string", "const": tool.source}
        }
    });
    let error_schema = json!({
        "type": "object", "additionalProperties": false, "required": ["error"],
        "properties": {"error": {"type": "object", "additionalProperties": false,
            "required": ["code", "message"], "properties": {"code": {"type": "string"}, "message": {"type": "string"}}}}
    });
    let endpoint = format!("/api/v1/tools/{}/query", tool.id);
    let operation = json!({
        "operationId": format!("query_{}", tool.id), "summary": tool.name, "description": tool.description,
        "security": [{"bearerAuth": []}, {"sessionCookie": []}],
        "parameters": [{"name": "X-Rayport-Request", "in": "header", "required": true,
            "schema": {"type": "string", "const": "1"}, "description": "요청 출처 확인을 위한 필수 헤더"}],
        "requestBody": {"required": true, "content": {"application/json": {"schema": input_schema}}},
        "responses": {
            "200": {"description": "검증된 조회 결과", "content": {"application/json": {"schema": {
                "type": "object", "additionalProperties": false, "required": ["data"],
                "properties": {"data": {"$ref": "#/components/schemas/QueryResult"}}
            }}}},
            "default": {"description": "인증·권한·입력·예산·원천·저장소 오류", "content": {"application/json": {
                "schema": {"$ref": "#/components/schemas/ApiError"}
            }}}
        },
        "x-rayport-tool-version": tool.version,
        "x-rayport-max-days": tool.max_days,
        "x-rayport-max-rows": tool.max_rows,
        "x-rayport-freshness-seconds": tool.freshness_seconds
    });
    let mut paths = serde_json::Map::new();
    paths.insert(endpoint, json!({"post": operation}));
    json!({
        "tool_id": tool.id, "version": tool.version, "publication_status": tool.status,
        "metadata_only": true, "mcp_transport_registered": false,
        "openapi": {
            "openapi": "3.1.1", "info": {"title": tool.name, "version": tool.version.to_string(),
                "description": "등록된 업무 계약에서 생성한 정의입니다. 초안은 게시 전 실행할 수 없습니다."},
            "servers": [{"url": origin}], "paths": paths,
            "components": {
                "securitySchemes": {
                    "bearerAuth": {"type": "http", "scheme": "bearer"},
                    "sessionCookie": {"type": "apiKey", "in": "cookie", "name": "rayport_session"}
                },
                "schemas": {"QueryInput": input_schema, "QueryResult": query_result, "ApiError": error_schema}
            }
        },
        "mcp_tool": {"name": tool.id, "title": tool.name, "description": tool.description,
            "inputSchema": input_schema, "annotations": {"readOnlyHint": true}}
    })
}
fn validate_source_result(
    r: &QueryResult,
    t: &Tool,
    q: &QueryInput,
    scope: &[String],
) -> Result<(), ApiError> {
    let invalid = || {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "invalid_source_response",
            "원천 응답의 범위·금액·완전성·신선도가 계약과 일치하지 않습니다.",
        )
    };
    if r.source != SourceKind::Apic || r.dataset_version == 0 || r.rows.len() > t.max_rows {
        return Err(invalid());
    }
    let start = parse_date(&q.start_date)?;
    let end = parse_date(&q.end_date_exclusive)?;
    validate_sales_rows(&r.rows).map_err(|_| invalid())?;
    for row in &r.rows {
        let day = parse_date(&row.business_date)?;
        if day < start || day >= end || !scope.contains(&row.store_id) {
            return Err(invalid());
        }
    }
    if r.rows.len() != (end - start).num_days() as usize * scope.len() {
        return Err(invalid());
    }
    let sum = aggregate_rows(&r.rows, q, scope).map_err(|_| invalid())?;
    if sum.net_minor != r.net_minor
        || sum.gross_minor != r.gross_minor
        || sum.cancel_minor != r.cancel_minor
        || r.currency != "KRW"
    {
        return Err(invalid());
    }
    let asof = DateTime::parse_from_rfc3339(&r.data_as_of)
        .map_err(|_| invalid())?
        .with_timezone(&Utc);
    let lag = Utc::now().signed_duration_since(asof).num_seconds();
    if lag < -300 || lag > t.freshness_seconds as i64 {
        return Err(invalid());
    }
    Ok(())
}
