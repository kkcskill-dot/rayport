use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Analyst,
    Viewer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub id: String,
    pub tenant_id: String,
    pub role: Role,
    pub store_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Mart,
    Apic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Draft,
    Validated,
    Published,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub source: SourceKind,
    pub source_ref: String,
    pub sql_template: Option<String>,
    pub status: ToolStatus,
    pub version: u32,
    pub max_days: u32,
    pub max_rows: usize,
    pub freshness_seconds: u64,
    pub owner: String,
    pub validation_evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryInput {
    pub start_date: String,
    pub end_date_exclusive: String,
    #[serde(default)]
    pub store_ids: Vec<String>,
}

/// All amounts are integer minor units. v1 permits KRW (one won) only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesRow {
    pub business_date: String,
    pub store_id: String,
    pub gross_minor: i64,
    pub cancel_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalesAggregate {
    pub rows: Vec<SalesRow>,
    pub net_minor: i64,
    pub gross_minor: i64,
    pub cancel_minor: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows: Vec<SalesRow>,
    pub net_minor: i64,
    pub gross_minor: i64,
    pub cancel_minor: i64,
    pub currency: String,
    pub dataset_version: u64,
    pub data_as_of: String,
    pub source: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqlInspection {
    pub normalized_sql: String,
    pub fingerprint: String,
    pub tables: Vec<String>,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainError {
    pub code: String,
    pub message: String,
}

impl DomainError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for DomainError {}
