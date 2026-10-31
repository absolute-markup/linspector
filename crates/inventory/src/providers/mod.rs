//! Shared package-provider contract. RPM and read-only dpkg adapters are implemented.
mod catalog;
pub(crate) mod dnf;
mod dpkg;
mod flatpak;
mod pacman;
mod rpm;
mod rpm_catalog;
pub(crate) mod rpm_query;
mod snap;
mod zypper;
use crate::{model::*, platform::Platform};
use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Unavailable,
    Unsupported,
    WrongProvider,
    NotFound,
    OperationFailed,
}
#[derive(Clone, Debug)]
pub struct ProviderError {
    pub kind: ErrorKind,
    pub message: String,
}
impl ProviderError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ProviderError {}
pub type Result<T> = std::result::Result<T, ProviderError>;
#[derive(Clone, Debug)]
pub struct Inventory {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
    pub source: String,
    pub provider_label: String,
    pub capabilities: ProviderCapabilities,
}
/// Not serializable or constructible by the QML client. Its implementation owns
/// any native plan files, verifies native state and cleans up on cancellation/drop.
/// Execution consumes the prepared plan, preventing accidental reuse.
/// ```compile_fail
/// use linspector_inventory::providers::PreparedRemoval;
/// fn execute_twice(plan: Box<dyn PreparedRemoval>) {
///     let _ = plan.execute();
///     let _ = plan.execute(); // the plan has already been consumed
/// }
/// ```
pub trait PreparedRemoval: Send {
    fn review(&self) -> &RemovalReview;
    fn execute(self: Box<Self>) -> Result<String>;
}
pub trait PackageProvider: Send + Sync {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn inventory(&self) -> Result<Inventory>;
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord>;
    fn dependencies(&self, id: &PackageId, include_reverse: bool) -> Result<DependencyReport>;
    /// Implementations must reject unsupported operations before creating plans.
    fn prepare_removal(&self, _id: &PackageId) -> Result<Box<dyn PreparedRemoval>> {
        Err(ProviderError::new(
            ErrorKind::Unsupported,
            "Removal preview is not supported by this provider",
        ))
    }
}
/// Factory selection is separate from capability authorization. Recognized RPM
/// systems can use inspection without acquiring DNF transactions on openSUSE.
pub fn select(platform: &Platform) -> Result<Box<dyn PackageProvider>> {
    if platform.capabilities.package_inventory
        && matches!(platform.family.as_str(), "dnf-rpm" | "zypper-rpm")
    {
        Ok(Box::new(rpm::RpmProvider::new(platform.clone())))
    } else if platform.capabilities.package_inventory && platform.family == "apt-dpkg" {
        Ok(Box::new(dpkg::DpkgProvider::system()))
    } else if platform.capabilities.package_inventory && platform.family == "pacman" {
        Ok(Box::new(pacman::PacmanProvider::system()))
    } else {
        Err(ProviderError::new(
            ErrorKind::Unavailable,
            if platform.capabilities.inventory_reason.is_empty() {
                "No implemented package provider is available".into()
            } else {
                platform.capabilities.inventory_reason.clone()
            },
        ))
    }
}
pub fn current() -> Result<Box<dyn PackageProvider>> {
    select(&crate::platform::detect())
}
pub(crate) fn demo_entries(packages: Vec<crate::Package>) -> Vec<Entry> {
    rpm::demo_entries(packages)
}
pub(crate) fn protected_service_package(name: &str) -> bool {
    rpm::protected_name(name)
}

/// Explicit read-only inspection of a mounted/extracted dpkg root for validation.
/// It does not execute the root's binaries, change the live provider, or permit mutations.
pub fn inspect_dpkg_root(root: &std::path::Path) -> Result<Box<dyn PackageProvider>> {
    Ok(Box::new(dpkg::DpkgProvider::offline(root)?))
}

/// Additional sources coexist with the native package manager.
pub fn flatpak(platform: &Platform) -> Result<Box<dyn PackageProvider>> {
    if !platform.capabilities.flatpak_inventory {
        return Err(ProviderError::new(
            ErrorKind::Unavailable,
            "Flatpak inventory is unavailable in this environment",
        ));
    }
    Ok(Box::new(flatpak::FlatpakProvider::current()?))
}
/// Route by the entry's provider, never by the host's default package manager alone.
pub fn for_package(platform: &Platform, id: &PackageId) -> Result<Box<dyn PackageProvider>> {
    if id.provider == "flatpak" {
        return flatpak(platform);
    }
    if id.provider == "snap" {
        return snap(platform);
    }
    let provider = select(platform)?;
    if provider.id() != id.provider {
        return Err(ProviderError::new(
            ErrorKind::WrongProvider,
            "Package source does not match the selected native provider",
        ));
    }
    Ok(provider)
}

pub fn snap(platform: &Platform) -> Result<Box<dyn PackageProvider>> {
    if !platform.capabilities.snap_inventory {
        return Err(ProviderError::new(
            ErrorKind::Unavailable,
            "Snap inventory is unavailable in this environment",
        ));
    }
    Ok(Box::new(snap::SnapProvider))
}
pub fn inspect_pacman_root(root: &std::path::Path) -> Result<Box<dyn PackageProvider>> {
    Ok(Box::new(pacman::PacmanProvider::offline(root)?))
}
pub fn inspect_snap_fixture(json: &str) -> Result<Inventory> {
    snap::parse(json, true)
}
