use crate::config::ApicConfig;
use rayport_domain::{Principal, QueryInput, QueryResult, Tool};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{future::Future, pin::Pin, time::Duration};

pub type SourceFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[derive(Clone, Debug)]
pub enum SourceError {
    Disabled,
    Uncertain,
    Rejected,
}
pub trait SourceExecutor: Send + Sync {
    fn enabled(&self) -> bool;
    fn execute<'a>(
        &'a self,
        id: &'a str,
        p: &'a Principal,
        t: &'a Tool,
        q: &'a QueryInput,
        scope: &'a [String],
    ) -> SourceFuture<'a, Result<QueryResult, SourceError>>;
    fn reconcile<'a>(&'a self, id: &'a str) -> SourceFuture<'a, bool>;
}
pub struct DisabledExecutor;
impl SourceExecutor for DisabledExecutor {
    fn enabled(&self) -> bool {
        false
    }
    fn execute<'a>(
        &'a self,
        _: &'a str,
        _: &'a Principal,
        _: &'a Tool,
        _: &'a QueryInput,
        _: &'a [String],
    ) -> SourceFuture<'a, Result<QueryResult, SourceError>> {
        Box::pin(async { Err(SourceError::Disabled) })
    }
    fn reconcile<'a>(&'a self, _: &'a str) -> SourceFuture<'a, bool> {
        Box::pin(async { false })
    }
}
pub struct HttpApicExecutor {
    config: ApicConfig,
    client: reqwest::Client,
    token: String,
}
#[derive(Serialize)]
struct ExecuteRequest<'a> {
    contract_version: &'static str,
    execution_id: &'a str,
    tool_id: &'a str,
    rayport_tool_version: u32,
    contract_hash: &'a str,
    approved_sql_fingerprint: &'a str,
    require_contract_match: bool,
    principal_id: &'a str,
    tenant_id: &'a str,
    authorized_store_ids: &'a [String],
    parameters: Parameters<'a>,
    timeout_seconds: u64,
}
#[derive(Serialize)]
struct Parameters<'a> {
    start_date: &'a str,
    end_date_exclusive: &'a str,
    store_id: &'a str,
}
#[derive(Deserialize)]
struct ExecuteResponse {
    contract_version: String,
    execution_id: String,
    contract_hash: String,
    approved_sql_fingerprint: String,
    terminated: bool,
    result: QueryResult,
}
#[derive(Deserialize)]
struct TerminationResponse {
    contract_version: String,
    execution_id: String,
    terminated: bool,
}
impl HttpApicExecutor {
    pub fn new(config: ApicConfig) -> Result<Self, String> {
        let token = std::env::var(&config.token_env)
            .map_err(|_| "APIC credential environment variable missing")?;
        if token.len() < 16 {
            return Err("APIC credential too short".into());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(config.timeout_seconds))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| "cannot build APIC client")?;
        Ok(Self {
            config,
            client,
            token,
        })
    }
    async fn bounded<T: serde::de::DeserializeOwned>(
        mut response: reqwest::Response,
    ) -> Result<T, SourceError> {
        if !response.status().is_success() {
            return Err(SourceError::Uncertain);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| SourceError::Uncertain)? {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err(SourceError::Uncertain);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| SourceError::Uncertain)
    }
    async fn confirmed(&self, url: &str, id: &str) -> bool {
        let result = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({"contract_version":"rayport-apic-v1","execution_id":id}))
            .send()
            .await;
        match result {
            Ok(r) => Self::bounded::<TerminationResponse>(r)
                .await
                .is_ok_and(|v| {
                    v.contract_version == "rayport-apic-v1" && v.execution_id == id && v.terminated
                }),
            Err(_) => false,
        }
    }
}
impl SourceExecutor for HttpApicExecutor {
    fn enabled(&self) -> bool {
        true
    }
    fn execute<'a>(
        &'a self,
        id: &'a str,
        p: &'a Principal,
        t: &'a Tool,
        q: &'a QueryInput,
        scope: &'a [String],
    ) -> SourceFuture<'a, Result<QueryResult, SourceError>> {
        Box::pin(async move {
            if scope.len() != 1 {
                return Err(SourceError::Rejected);
            }
            let binding = approved_binding(t)?;
            let payload = ExecuteRequest {
                contract_version: "rayport-apic-v1",
                execution_id: id,
                tool_id: &t.source_ref,
                rayport_tool_version: t.version,
                contract_hash: &binding.contract_hash,
                approved_sql_fingerprint: &binding.approved_sql_fingerprint,
                require_contract_match: true,
                principal_id: &p.id,
                tenant_id: &p.tenant_id,
                authorized_store_ids: scope,
                parameters: Parameters {
                    start_date: &q.start_date,
                    end_date_exclusive: &q.end_date_exclusive,
                    store_id: &scope[0],
                },
                timeout_seconds: self.config.timeout_seconds,
            };
            let response = self
                .client
                .post(&self.config.execute_url)
                .bearer_auth(&self.token)
                .json(&payload)
                .send()
                .await;
            let result = match response {
                Ok(r) => Self::bounded::<ExecuteResponse>(r).await,
                Err(_) => Err(SourceError::Uncertain),
            };
            match result {
                Ok(v)
                    if v.contract_version == "rayport-apic-v1"
                        && v.execution_id == id
                        && v.terminated
                        && v.contract_hash == binding.contract_hash
                        && v.approved_sql_fingerprint == binding.approved_sql_fingerprint =>
                {
                    Ok(v.result)
                }
                _ => {
                    if self.confirmed(&self.config.cancel_url, id).await {
                        Err(SourceError::Rejected)
                    } else {
                        Err(SourceError::Uncertain)
                    }
                }
            }
        })
    }
    fn reconcile<'a>(&'a self, id: &'a str) -> SourceFuture<'a, bool> {
        Box::pin(async move { self.confirmed(&self.config.status_url, id).await })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovedBinding {
    pub material: serde_json::Value,
    pub contract_hash: String,
    pub approved_sql_fingerprint: String,
}
/// Sorted JSON keys, UTF-8, no whitespace. The remote adapter must verify this binding BEFORE execution.
pub fn approved_binding(tool: &Tool) -> Result<ApprovedBinding, SourceError> {
    let sql = tool.sql_template.as_deref().ok_or(SourceError::Rejected)?;
    let inspection = rayport_domain::inspect_sql(sql).map_err(|_| SourceError::Rejected)?;
    let material = serde_json::json!({"contract_version":"rayport-apic-v1","tool_id":tool.source_ref,"rayport_tool_id":tool.id,"rayport_tool_version":tool.version,"sql_fingerprint":inspection.fingerprint,"max_days":tool.max_days,"max_rows":tool.max_rows,"freshness_seconds":tool.freshness_seconds,"scope_parameter":"store_id","start_parameter":"start_date","end_parameter":"end_date_exclusive","currency":"KRW"});
    let bytes = serde_json::to_vec(&material).map_err(|_| SourceError::Rejected)?;
    Ok(ApprovedBinding {
        contract_hash: format!("{:x}", Sha256::digest(bytes)),
        approved_sql_fingerprint: inspection.fingerprint,
        material,
    })
}
