//! Infrastructure adapters. This crate does not start services or databases.

#[path = "persistence/CratesRayportAdaptersSrcPersistenceSnapshotStore.rs"]
mod snapshot_store;

pub use snapshot_store::{SnapshotStore, StoreError};
