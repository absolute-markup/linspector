//! Native DNF5 removal-plan implementation owned by the RPM adapter.
//! No action is performed while scanning or generating a plan.
use crate::Package;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemovalRow {
    pub nevra: String,
    pub action: String,
    pub reason: String,
}
#[derive(Deserialize)]
struct Transaction {
    version: String,
    rpms: Vec<RemovalRow>,
    #[serde(default)]
    groups: Vec<serde_json::Value>,
    #[serde(default)]
    environments: Vec<serde_json::Value>,
}
pub struct RemovalPlan {
    directory: PathBuf,
    bytes: Vec<u8>,
    pub rows: Vec<RemovalRow>,
    pub requested: String,
    expected_installed: Vec<String>,
    expected_removals: Vec<String>,
}
impl Drop for RemovalPlan {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn valid(p: &Package) -> Result<(), String> {
    if p.provenance.starts_with("Synthetic") {
        return Err("System actions are unavailable in demo mode".into());
    }
    if p.arch == "unknown" || p.name == "gpg-pubkey" {
        return Err("RPM key records cannot be removed here".into());
    }
    Ok(())
}
pub(super) fn protected(name: &str) -> bool {
    if name.starts_with("qt6-") && (name.ends_with("-devel") || name.ends_with("-private-devel")) {
        return false;
    }
    [
        "kernel",
        "glibc",
        "systemd",
        "dnf",
        "dnf5",
        "rpm",
        "sudo",
        "polkit",
        "qt6-qtbase",
        "qt6-qtdeclarative",
        "linspector",
    ]
    .iter()
    .any(|n| name == *n || name.starts_with(&format!("{n}-")))
}
fn stored_identity(p: &Package) -> String {
    if p.epoch == "0" {
        format!("{}-{}-{}.{}", p.name, p.version, p.release, p.arch)
    } else {
        p.identity()
    }
}
/// Reject non-removal actions, unknown packages, protected packages and empty plans.
fn validate(
    bytes: &[u8],
    installed: &[Package],
    requested: &Package,
) -> Result<Vec<RemovalRow>, String> {
    let tx: Transaction =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid DNF plan: {e}"))?;
    if tx.version != "1.0"
        || tx.rpms.is_empty()
        || !tx.groups.is_empty()
        || !tx.environments.is_empty()
    {
        return Err("Unsupported or empty DNF transaction".into());
    }
    let mut found = false;
    let mut seen = std::collections::BTreeSet::new();
    for row in &tx.rpms {
        if row.action != "Remove" {
            return Err("Only removal-only transactions can be reviewed here".into());
        }
        let p = installed
            .iter()
            .find(|p| stored_identity(p) == row.nevra || p.identity() == row.nevra)
            .ok_or_else(|| format!("Unrecognized package in plan: {}", row.nevra))?;
        if !seen.insert(p.identity()) {
            return Err("Duplicated removal in DNF transaction".into());
        }
        if protected(&p.name) {
            return Err(format!(
                "Removal blocked: {} is a core system or Linspector runtime package",
                p.name
            ));
        }
        if p.identity() == requested.identity() {
            found = true;
        }
    }
    if !found {
        return Err("Selected package is not in DNF's transaction".into());
    }
    Ok(tx.rpms)
}
pub fn preview(p: &Package) -> Result<RemovalPlan, String> {
    preview_in(p, std::env::temp_dir())
}
fn authorize_preview(platform: &crate::platform::Platform) -> Result<(), String> {
    if platform.family == "dnf-rpm" && platform.capabilities.removal_preview {
        Ok(())
    } else {
        Err("DNF review is unavailable for this host package manager".into())
    }
}
fn preview_in(p: &Package, base: PathBuf) -> Result<RemovalPlan, String> {
    valid(p)?;
    authorize_preview(&crate::platform::detect())?;
    if protected(&p.name) {
        return Err("Core system and Linspector runtime packages are protected".into());
    }
    let installed = crate::installed()?;
    if !installed.iter().any(|x| x.identity() == p.identity()) {
        return Err("Package changed since the scan. Refresh first.".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = base.join(format!("linspector-removal-{}-{stamp}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .map_err(|e| e.to_string())?;
    let mut plan = RemovalPlan {
        directory: dir,
        bytes: vec![],
        rows: vec![],
        requested: p.identity(),
        expected_installed: installed.iter().map(Package::identity).collect(),
        expected_removals: vec![],
    };
    let log = format!("logdir={}", plan.directory.display());
    let store = format!("--store={}", plan.directory.display());
    // --store resolves and writes a plan; it never executes the transaction.
    let out = crate::process::run(
        crate::process::command("/usr/bin/dnf5").args([
            "--disable-repo=*",
            "--setopt",
            &log,
            "--assumeyes",
            "remove",
            "--no-autoremove",
            &store,
            "--",
            &p.identity(),
        ]),
        Some(std::time::Duration::from_secs(45)),
    )?;
    if out.timed_out {
        return Err("DNF removal preview timed out; no transaction was executed".into());
    }
    if !out.success() || out.truncated {
        return Err(format!(
            "DNF could not prepare a removal plan:\n{}\n{}",
            out.stdout, out.stderr
        ));
    }
    plan.bytes = fs::read(plan.directory.join("transaction.json"))
        .map_err(|e| format!("No stored DNF transaction: {e}"))?;
    plan.rows = validate(&plan.bytes, &installed, p)?;
    plan.expected_removals = plan
        .rows
        .iter()
        .map(|r| {
            installed
                .iter()
                .find(|p| stored_identity(p) == r.nevra || p.identity() == r.nevra)
                .unwrap()
                .identity()
        })
        .collect();
    Ok(plan)
}
/// Called only after the user confirms the complete native transaction review.
pub fn execute(plan: RemovalPlan) -> Result<String, String> {
    crate::platform::require_removal()?;
    if fs::read(plan.directory.join("transaction.json")).map_err(|e| e.to_string())? != plan.bytes {
        return Err("Stored plan changed. Review a new plan.".into());
    }
    crate::privileged::invoke(&crate::privileged::Request {
        version: 1,
        action: crate::privileged::Action::Remove {
            requested: crate::model::PackageId {
                provider: "rpm".into(),
                scope: "system".into(),
                native: plan.requested.clone(),
            },
            expected_installed: plan.expected_installed.clone(),
            expected_removals: plan.expected_removals.clone(),
        },
    })
}
/// Root-only, fresh planning. Client-owned transaction files are never replayed.
pub(crate) fn root_remove(
    p: &Package,
    expected: &[String],
    expected_installed: &[String],
) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let base = PathBuf::from("/run/linspector");
    match fs::DirBuilder::new().mode(0o700).create(&base) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    let meta = fs::symlink_metadata(&base).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != 0 || meta.permissions().mode() & 0o077 != 0 {
        return Err("Management working directory must be root-owned and private".into());
    }
    for parent in base.ancestors().skip(1) {
        let meta = fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.uid() != 0 || meta.permissions().mode() & 0o022 != 0 {
            return Err("Management working directory has an untrusted parent".into());
        }
    }
    let plan = preview_in(p, base)?;
    let set = |v: &[String]| v.iter().cloned().collect::<std::collections::BTreeSet<_>>();
    if set(&plan.expected_installed) != set(expected_installed)
        || set(&plan.expected_removals) != set(expected)
    {
        return Err(
            "Installed state or native solver result changed. Review a new removal plan.".into(),
        );
    }
    let out = crate::process::run(
        crate::process::command("/usr/bin/dnf5")
            .args(["--disable-repo=*", "--assumeyes", "replay"])
            .arg(&plan.directory),
        None,
    )?;
    if !out.success() {
        return Err(format!(
            "Removal cancelled or failed:\n{}\n{}",
            out.stdout, out.stderr
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packages() -> Vec<Package> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sample-inventory.json"
        )))
        .unwrap()
    }
    #[test]
    fn zypper_preview_capability_never_authorizes_legacy_dnf_review() {
        let platform = crate::platform::identify(&crate::platform::Facts {
            os_release: "ID=opensuse-tumbleweed\n".into(),
            tools: ["rpm", "zypper", "dnf5"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        });
        assert!(platform.capabilities.removal_preview);
        assert!(authorize_preview(&platform).is_err());
    }
    #[test]
    fn plan_rejects_installs_and_unrecognized_or_protected_removals() {
        let p = packages();
        let selected = &p[0];
        let tx = |nevra: &str, action: &str| {
            serde_json::to_vec(&serde_json::json!({"version":"1.0","rpms":[{"nevra":nevra,"action":action,"reason":"User"}]})).unwrap()
        };
        assert!(validate(&tx(&stored_identity(selected), "Install"), &p, selected).is_err());
        assert!(validate(&tx("not-installed-1-1.x86_64", "Remove"), &p, selected).is_err());
        assert!(validate(&tx(&stored_identity(selected), "Remove"), &p, selected).is_ok());
        let repeated = serde_json::to_vec(&serde_json::json!({"version":"1.0","rpms":[
            {"nevra":stored_identity(selected),"action":"Remove","reason":"User"},
            {"nevra":selected.identity(),"action":"Remove","reason":"User"}
        ]}))
        .unwrap();
        assert!(validate(&repeated, &p, selected).is_err());
        let mut core = selected.clone();
        core.name = "systemd".into();
        assert!(validate(
            &tx(&stored_identity(&core), "Remove"),
            &[core.clone()],
            &core
        )
        .is_err());
    }
    #[test]
    fn demo_actions_blocked_and_protection_is_name_exact() {
        assert!(valid(&packages()[0]).is_err());
        assert!(protected("glibc-common"));
        assert!(!protected("systemdgenie"));
    }
    #[test]
    fn discarding_native_plan_removes_its_private_directory() {
        use std::os::unix::fs::PermissionsExt;
        let directory = std::env::temp_dir().join(format!(
            "linspector-plan-cleanup-test-{}",
            std::process::id()
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::write(directory.join("transaction.json"), b"fixture").unwrap();
        let plan = RemovalPlan {
            directory: directory.clone(),
            bytes: vec![],
            rows: vec![],
            requested: String::new(),
            expected_installed: vec![],
            expected_removals: vec![],
        };
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        drop(plan);
        assert!(!directory.exists());
    }
}
