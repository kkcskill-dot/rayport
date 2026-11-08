//! Framework-free use cases and persisted single-tenant workspace.
#[path = "CratesRayportApplicationSrcWorkspace.rs"]
mod workspace;
pub use workspace::*;

#[cfg(test)]
#[path = "CratesRayportApplicationSrcWorkspaceTests.rs"]
mod tests;

#[path = "catalog/CratesRayportApplicationSrcCatalogToolUseCases.rs"]
mod catalog;

#[path = "discovery/CratesRayportApplicationSrcDiscoveryUseCases.rs"]
mod discovery;

#[path = "mart/CratesRayportApplicationSrcMartUseCases.rs"]
mod mart;

#[path = "request/CratesRayportApplicationSrcRequestUseCases.rs"]
mod request;

#[path = "query/CratesRayportApplicationSrcQueryUseCases.rs"]
mod query;

#[path = "governance/CratesRayportApplicationSrcGovernanceUseCases.rs"]
mod governance;
