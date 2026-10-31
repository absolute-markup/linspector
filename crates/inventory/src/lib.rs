//! Linux inventory engine: portable discovery and selected native software providers.
pub mod host;
pub mod management;
pub mod model;
pub mod platform;
pub mod privileged;
mod process;
pub mod providers;
pub mod review;
pub mod services;
pub mod system;
// Compatibility exports for the original RPM JSON CLI and fixture tests.
pub use providers::rpm_query::{
    installed, parse_rpm_xml, query_format, reverse_candidates, Package,
};
