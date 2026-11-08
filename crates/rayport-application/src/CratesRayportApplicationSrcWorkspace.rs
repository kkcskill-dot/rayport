use rayport_domain::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type AppResult<T> = Result<T, DomainError>;
pub(crate) fn fail(code: &str, message: &str) -> DomainError {
    DomainError::new(code, message)
}
pub(crate) fn id(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub paused: bool,
    pub per_user_per_minute: usize,
    pub source_per_minute: usize,
    pub max_concurrent: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            paused: false,
            per_user_per_minute: 10,
            source_per_minute: 20,
            max_concurrent: 2,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub actor: String,
    pub action: String,
    pub object_id: String,
    pub at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub source_label: String,
    pub observed_at: String,
    pub observation_version: u64,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub id: String,
    pub name: String,
    pub sql_template: String,
    pub signature: String,
    pub executions: Option<u64>,
    pub ash_samples: Option<u64>,
    pub module: Option<String>,
    pub converted_tool_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub sql_text: String,
    pub executions: Option<u64>,
    pub ash_samples: Option<u64>,
    pub module: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryImport {
    #[serde(default)]
    pub window_start: Option<String>,
    #[serde(default)]
    pub window_end: Option<String>,
    pub source_label: String,
    pub diagnostics_authorized: bool,
    pub observations: Vec<Observation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    #[serde(default)]
    pub ready_after_execution_sequence: Option<u64>,
    #[serde(default)]
    pub ready_at_ms: Option<i64>,
    pub id: String,
    pub question: String,
    pub purpose: String,
    pub status: String,
    pub requester_id: String,
    pub linked_tool_id: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MartImport {
    pub id: String,
    pub name: String,
    pub data_as_of: String,
    pub complete_through: String,
    pub rows: Vec<SalesRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mart {
    pub id: String,
    pub name: String,
    pub versions: Vec<MartVersion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MartVersion {
    pub version: u64,
    pub data_as_of: String,
    pub complete_through: String,
    pub rows: Vec<SalesRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MartSummary {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub rows_count: usize,
    pub data_as_of: String,
    pub complete_through: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub completed_at_ms: Option<i64>,
    pub id: String,
    pub actor: String,
    pub tool_id: String,
    pub source_ref: String,
    pub source: SourceKind,
    pub status: String,
    pub started_at: i64,
    pub charged_days: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub next_execution_sequence: u64,
    pub schema_version: u32,
    pub tenant_id: String,
    pub tools: Vec<Tool>,
    pub candidates: Vec<Candidate>,
    pub requests: Vec<CapabilityRequest>,
    pub marts: Vec<Mart>,
    pub policy: Policy,
    pub audit: Vec<AuditEvent>,
    pub executions: Vec<Execution>,
    pub blocked_executions: u64,
    pub completed_executions: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Operations {
    pub active_executions: usize,
    pub blocked_executions: u64,
    pub completed_executions: u64,
    pub source_execution_enabled: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceView {
    pub tools: Vec<Tool>,
    pub candidates: Vec<Candidate>,
    pub requests: Vec<CapabilityRequest>,
    pub marts: Vec<MartSummary>,
    pub policy: Policy,
    pub audit: Vec<AuditEvent>,
    pub operations: Operations,
}
#[derive(Debug, Clone, Serialize)]
pub struct Resolution {
    pub status: String,
    pub tool_id: Option<String>,
    pub explanation: String,
    pub suggestions: Vec<String>,
}

impl Workspace {
    pub fn new(tenant_id: String) -> Self {
        Self {
            next_execution_sequence: 0,
            schema_version: 1,
            tenant_id,
            tools: vec![],
            candidates: vec![],
            requests: vec![],
            marts: vec![],
            policy: Policy::default(),
            audit: vec![],
            executions: vec![],
            blocked_executions: 0,
            completed_executions: 0,
        }
    }

    pub fn view(&self, p: &Principal, apic_enabled: bool) -> AppResult<WorkspaceView> {
        self.authorize(p)?;
        let admin = p.role == Role::Admin;
        let tools = self
            .tools
            .iter()
            .filter(|t| admin || t.status == ToolStatus::Published)
            .cloned()
            .map(|mut t| {
                if !admin {
                    t.sql_template = None;
                    t.validation_evidence = None;
                }
                t
            })
            .collect();
        Ok(WorkspaceView {
            tools,
            candidates: if admin {
                self.candidates.clone()
            } else {
                vec![]
            },
            requests: self
                .requests
                .iter()
                .filter(|r| admin || r.requester_id == p.id)
                .cloned()
                .collect(),
            marts: if admin {
                self.marts.iter().map(Mart::summary).collect()
            } else {
                vec![]
            },
            policy: self.policy.clone(),
            audit: if admin {
                self.audit.iter().rev().take(200).cloned().collect()
            } else {
                vec![]
            },
            operations: Operations {
                active_executions: self
                    .executions
                    .iter()
                    .filter(|e| is_active(&e.status))
                    .count(),
                blocked_executions: self.blocked_executions,
                completed_executions: self.completed_executions,
                source_execution_enabled: apic_enabled,
            },
        })
    }
}
pub(crate) fn is_active(status: &str) -> bool {
    matches!(status, "running" | "in_doubt")
}
impl Mart {
    pub fn latest(&self) -> &MartVersion {
        self.versions
            .last()
            .expect("persisted mart must have a complete version")
    }
    pub fn summary(&self) -> MartSummary {
        let v = self.latest();
        MartSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            version: v.version,
            rows_count: v.rows.len(),
            data_as_of: v.data_as_of.clone(),
            complete_through: v.complete_through.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDraftCommand {
    pub name: String,
    pub source_ref: String,
    pub source: SourceKind,
    pub owner: String,
}
