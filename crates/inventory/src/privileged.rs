//! Narrow, versioned management requests. No client-supplied command or plan path.
use crate::{model::PackageId, Package};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    time::Duration,
};
pub const HELPER: &str = "/usr/libexec/linspector-helper";
pub const POLICY: &str = "/usr/share/polkit-1/actions/com.absolutemarkup.linspector.policy";
pub const MAX_REQUEST: usize = 8 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub action: Action,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Remove {
        requested: PackageId,
        expected_installed: Vec<String>,
        expected_removals: Vec<String>,
    },
    ServiceStartup {
        package: PackageId,
        unit: String,
        expected_state: String,
        enable: bool,
    },
}
pub fn parse_request(bytes: &[u8]) -> Result<Request, String> {
    if bytes.len() > MAX_REQUEST {
        return Err("Management request is too large".into());
    }
    let request: Request =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid management request: {e}"))?;
    if request.version != 1 {
        return Err("Unsupported management protocol".into());
    }
    match &request.action {
        Action::Remove {
            requested,
            expected_installed,
            expected_removals,
        } => {
            check_id(requested)?;
            if expected_installed.is_empty()
                || expected_removals.is_empty()
                || expected_removals.len() > expected_installed.len()
            {
                return Err("Empty or inconsistent reviewed removal".into());
            }
            unique(expected_installed)?;
            unique(expected_removals)?;
        }
        Action::ServiceStartup {
            package,
            unit,
            expected_state,
            enable,
        } => {
            check_id(package)?;
            crate::services::valid_unit(unit)?;
            if expected_state != if *enable { "disabled" } else { "enabled" } {
                return Err("Service state does not match the reviewed startup change".into());
            }
        }
    }
    Ok(request)
}
fn unique(values: &[String]) -> Result<(), String> {
    if values
        .iter()
        .any(|v| v.is_empty() || v.len() > 4096 || v.contains('\0'))
        || values.iter().collect::<BTreeSet<_>>().len() != values.len()
    {
        return Err("Invalid or duplicated reviewed identity".into());
    }
    Ok(())
}
fn check_id(id: &PackageId) -> Result<(), String> {
    if id.provider != "rpm"
        || id.scope != "system"
        || id.native.is_empty()
        || id.native.len() > 4096
        || id.native.contains('\0')
    {
        return Err("Management requires an exact installed system RPM identity".into());
    }
    Ok(())
}
pub(crate) fn trusted_file(path: &Path, executable: bool) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_file()
        || meta.uid() != 0
        || meta.permissions().mode() & 0o022 != 0
        || (executable && meta.permissions().mode() & 0o111 == 0)
    {
        return false;
    }
    let mut parent = path.parent();
    while let Some(path) = parent {
        let Ok(meta) = fs::symlink_metadata(path) else {
            return false;
        };
        if !meta.is_dir() || meta.uid() != 0 || meta.permissions().mode() & 0o022 != 0 {
            return false;
        }
        parent = path.parent();
    }
    true
}
pub fn available() -> bool {
    trusted_file(Path::new(HELPER), true)
        && trusted_file(Path::new(POLICY), false)
        && fs::read(POLICY).ok().as_deref()
            == Some(
                include_bytes!("../../../packaging/polkit/com.absolutemarkup.linspector.policy")
                    .as_slice(),
            )
}
pub(crate) fn invoke(request: &Request) -> Result<String, String> {
    if !available() {
        return Err("The trusted management helper/policy is not installed. Read-only inspection and removal previews remain available.".into());
    }
    let bytes = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    parse_request(&bytes)?;
    let output = crate::process::run_input(
        crate::process::command("/usr/bin/pkexec").args(["--disable-internal-agent", HELPER]),
        None,
        Some(bytes),
    )?;
    classify(output.code, &output.stdout, &output.stderr)
}

pub(crate) fn classify(code: Option<i32>, stdout: &str, stderr: &str) -> Result<String, String> {
    match code {
        Some(0)=>Ok(stdout.trim().into()),
        Some(126)=>Err("Authentication was cancelled. Refreshing the observed system state; no automatic retry will run.".into()),
        Some(127)=>Err("Authorization was denied or the authentication agent/helper could not be reached. Refreshing the observed state.".into()),
        _=>Err(format!("Management did not complete successfully ({code:?}). State may have changed; inspect the refreshed inventory before retrying.\n{}",stderr.trim())),
    }
}
/// Testable pre/postcondition engine. Native execution is an implementation detail,
/// and tests use an in-memory host, never a real package mutation.
pub(crate) trait RemovalHost {
    fn installed(&mut self) -> Result<Vec<Package>, String>;
    fn remove(
        &mut self,
        requested: &Package,
        expected: &[String],
        installed: &[String],
    ) -> Result<(), String>;
}
pub(crate) fn remove_with(
    host: &mut impl RemovalHost,
    request: &Request,
) -> Result<String, String> {
    let Action::Remove {
        requested,
        expected_installed,
        expected_removals,
    } = &request.action
    else {
        return Err("Expected removal request".into());
    };
    parse_request(&serde_json::to_vec(request).map_err(|e| e.to_string())?)?;
    let before = host.installed()?;
    let identities: BTreeSet<_> = before.iter().map(Package::identity).collect();
    if identities != expected_installed.iter().cloned().collect() {
        return Err("Installed packages changed after review. Review a new removal plan.".into());
    }
    let package = before
        .iter()
        .find(|p| p.identity() == requested.native)
        .ok_or("Selected package is no longer installed")?;
    if crate::providers::protected_service_package(&package.name) {
        return Err("Core system and Linspector runtime packages are protected".into());
    }
    if !expected_removals.contains(&requested.native)
        || !expected_removals.iter().all(|id| identities.contains(id))
    {
        return Err("Reviewed removals do not match installed identities".into());
    }
    if before.iter().any(|p| {
        expected_removals.contains(&p.identity())
            && crate::providers::protected_service_package(&p.name)
    }) {
        return Err("Reviewed removal includes protected system/runtime packages".into());
    }
    let execution = host.remove(package, expected_removals, expected_installed);
    let removed: BTreeSet<_> = expected_removals.iter().cloned().collect();
    let expected: BTreeSet<_> = identities.difference(&removed).cloned().collect();
    let after = host.installed().map_err(|error|format!("Operation outcome could not be verified. State may have changed; rescan before retrying. {error}"))?;
    let after: BTreeSet<_> = after.iter().map(Package::identity).collect();
    if let Err(error) = execution {
        return Err(format!("{error}\nObserved installed identities changed: {}. Rescan and review again; no automatic retry or rollback was attempted.",after!=identities));
    }
    if after != expected {
        return Err("Removal post-check differs from the reviewed result. State may have changed; rescan and inspect DNF history. No automatic rollback was attempted.".into());
    }
    Ok("Removal completed and installed package identities matched the reviewed result.".into())
}
struct Native;
impl RemovalHost for Native {
    fn installed(&mut self) -> Result<Vec<Package>, String> {
        crate::installed()
    }
    fn remove(
        &mut self,
        package: &Package,
        expected: &[String],
        installed: &[String],
    ) -> Result<(), String> {
        crate::providers::dnf::root_remove(package, expected, installed)
    }
}
/// Entry point for the installed helper only. Never called by the desktop in-process.
pub fn helper_main() -> Result<String, String> {
    if std::env::args_os().len() != 1 {
        return Err("The helper accepts no command-line arguments".into());
    }
    let status = fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let uid = status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .ok_or("Cannot establish helper credentials")?;
    if uid.split_whitespace().take(2).any(|id| id != "0") {
        return Err("The helper requires root authentication through pkexec".into());
    }
    std::env::var("PKEXEC_UID")
        .map_err(|_| "Missing polkit caller identity")?
        .parse::<u32>()
        .map_err(|_| "Invalid polkit caller identity")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = std::io::stdin()
            .take(MAX_REQUEST as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    let bytes = rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| "Management request timed out")??;
    let request = parse_request(&bytes)?;
    match &request.action {
        Action::Remove { .. } => {
            crate::platform::require_removal()?;
            remove_with(&mut Native, &request)
        }
        Action::ServiceStartup {
            package,
            unit,
            expected_state,
            enable,
        } => {
            crate::platform::require_service_control()?;
            crate::services::root_change_startup(package, unit, expected_state, *enable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packages() -> Vec<Package> {
        let mut packages: Vec<Package> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sample-inventory.json"
        )))
        .unwrap();
        for p in &mut packages {
            p.provenance = "Test-only in-memory host".into();
        }
        packages
    }
    fn request(packages: &[Package]) -> Request {
        Request {
            version: 1,
            action: Action::Remove {
                requested: PackageId {
                    provider: "rpm".into(),
                    scope: "system".into(),
                    native: packages[0].identity(),
                },
                expected_installed: packages.iter().map(Package::identity).collect(),
                expected_removals: vec![packages[0].identity()],
            },
        }
    }
    #[derive(Clone, Copy)]
    enum Behavior {
        Success,
        NoChange,
        Fail,
        PartialFailure,
        SolverChanged,
        LostPostcheck,
    }
    struct Fake {
        packages: Vec<Package>,
        behavior: Behavior,
        calls: usize,
        reads: usize,
    }
    impl Fake {
        fn new(behavior: Behavior) -> Self {
            Self {
                packages: packages(),
                behavior,
                calls: 0,
                reads: 0,
            }
        }
    }
    impl RemovalHost for Fake {
        fn installed(&mut self) -> Result<Vec<Package>, String> {
            self.reads += 1;
            if matches!(self.behavior, Behavior::LostPostcheck) && self.reads > 1 {
                return Err("injected RPM read failure".into());
            }
            Ok(self.packages.clone())
        }
        fn remove(&mut self, _: &Package, expected: &[String], _: &[String]) -> Result<(), String> {
            self.calls += 1;
            match self.behavior {
                Behavior::Success | Behavior::LostPostcheck => {
                    self.packages.retain(|p| !expected.contains(&p.identity()));
                    Ok(())
                }
                Behavior::NoChange => Ok(()),
                Behavior::Fail => Err("injected DNF lock/native failure".into()),
                Behavior::SolverChanged => Err("native solver result changed; review again".into()),
                Behavior::PartialFailure => {
                    self.packages.retain(|p| !expected.contains(&p.identity()));
                    Err("injected interrupted transaction".into())
                }
            }
        }
    }
    #[test]
    fn exact_review_succeeds_only_after_postcheck() {
        let mut host = Fake::new(Behavior::Success);
        let request = request(&host.packages);
        assert!(remove_with(&mut host, &request)
            .unwrap()
            .contains("matched"));
        assert_eq!(host.calls, 1);
        assert_eq!(host.reads, 2);
    }
    #[test]
    fn stale_inventory_blocks_execution_before_native_call() {
        let mut host = Fake::new(Behavior::Success);
        let request = request(&host.packages);
        host.packages[0].version = "changed".into();
        assert!(remove_with(&mut host, &request)
            .unwrap_err()
            .contains("changed after review"));
        assert_eq!(host.calls, 0);
    }
    #[test]
    fn protected_dependency_or_foreign_source_cannot_be_removed() {
        let mut host = Fake::new(Behavior::Success);
        host.packages[1].name = "systemd".into();
        let mut request = request(&host.packages);
        if let Action::Remove {
            expected_removals, ..
        } = &mut request.action
        {
            expected_removals.push(host.packages[1].identity());
        }
        assert!(remove_with(&mut host, &request)
            .unwrap_err()
            .contains("protected"));
        assert_eq!(host.calls, 0);
        if let Action::Remove { requested, .. } = &mut request.action {
            requested.provider = "dpkg".into();
        }
        assert!(remove_with(&mut host, &request).is_err());
        assert_eq!(host.calls, 0);
    }
    #[test]
    fn unsuccessful_native_call_is_observed_and_never_retried() {
        for behavior in [
            Behavior::Fail,
            Behavior::PartialFailure,
            Behavior::SolverChanged,
        ] {
            let mut host = Fake::new(behavior);
            let request = request(&host.packages);
            let message = remove_with(&mut host, &request).unwrap_err();
            assert!(message.contains("no automatic retry"));
            if matches!(behavior, Behavior::PartialFailure) {
                assert!(message.contains("changed: true"));
            }
            assert_eq!(host.calls, 1);
            assert_eq!(host.reads, 2);
        }
    }
    #[test]
    fn zero_exit_without_expected_changes_and_lost_postcheck_are_not_success() {
        for behavior in [Behavior::NoChange, Behavior::LostPostcheck] {
            let mut host = Fake::new(behavior);
            let request = request(&host.packages);
            assert!(remove_with(&mut host, &request).is_err());
            assert_eq!(host.calls, 1);
        }
    }
    #[test]
    fn request_rejects_paths_commands_unknown_fields_and_versions() {
        let mut data = serde_json::to_value(request(&packages())).unwrap();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_ok());
        data["action"]["plan_path"] = "/tmp/attacker".into();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_err());
        data["action"].as_object_mut().unwrap().remove("plan_path");
        data["action"]["requested"]["command"] = "sh".into();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_err());
        data["action"]["requested"]
            .as_object_mut()
            .unwrap()
            .remove("command");
        data["version"] = 2.into();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(parse_request(&vec![b' '; MAX_REQUEST + 1]).is_err());
    }
    #[test]
    fn empty_duplicate_and_unreviewed_identities_are_rejected() {
        let mut host = Fake::new(Behavior::Success);
        let mut r = request(&host.packages);
        if let Action::Remove {
            expected_removals, ..
        } = &mut r.action
        {
            expected_removals.clear();
        }
        assert!(remove_with(&mut host, &r).is_err());
        if let Action::Remove {
            expected_removals, ..
        } = &mut r.action
        {
            expected_removals.extend([host.packages[0].identity(), host.packages[0].identity()]);
        }
        assert!(remove_with(&mut host, &r).is_err());
        if let Action::Remove {
            expected_removals, ..
        } = &mut r.action
        {
            *expected_removals = vec!["unreviewed-1-1.x86_64".into()];
        }
        assert!(remove_with(&mut host, &r).is_err());
        assert_eq!(host.calls, 0);
    }
    #[test]
    fn authentication_cancellation_denial_and_signal_have_distinct_messages() {
        assert!(classify(Some(126), "", "")
            .unwrap_err()
            .contains("cancelled"));
        assert!(classify(Some(127), "", "").unwrap_err().contains("denied"));
        assert!(classify(None, "", "")
            .unwrap_err()
            .contains("State may have changed"));
        assert_eq!(classify(Some(0), "completed\n", "").unwrap(), "completed");
    }
    #[test]
    fn service_request_is_narrow_and_state_bound() {
        let id = PackageId {
            provider: "rpm".into(),
            scope: "system".into(),
            native: "test-1-1.x86_64".into(),
        };
        let request = Request {
            version: 1,
            action: Action::ServiceStartup {
                package: id,
                unit: "example.service".into(),
                expected_state: "disabled".into(),
                enable: true,
            },
        };
        let mut data = serde_json::to_value(request).unwrap();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_ok());
        data["action"]["expected_state"] = "enabled".into();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_err());
        data["action"]["unit"] = "../../arbitrary.service".into();
        assert!(parse_request(&serde_json::to_vec(&data).unwrap()).is_err());
    }
    #[test]
    fn missing_or_untrusted_helper_has_no_direct_command_fallback() {
        assert!(!trusted_file(
            Path::new("/tmp/no-installed-linspector-helper"),
            true
        ));
        if !available() {
            assert!(invoke(&request(&packages()))
                .unwrap_err()
                .contains("not installed"));
        }
    }
}
