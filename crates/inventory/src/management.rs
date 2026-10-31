//! Compatibility facade for the original RPM-specific API.
//! The application uses `providers::PackageProvider`; these exports preserve the CLI/tests API.
pub use crate::providers::dnf::{execute, preview, RemovalPlan, RemovalRow};
pub use crate::services::service_state;
