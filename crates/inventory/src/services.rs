//! Exact package-owned systemd startup operations, separate from package removal.
use crate::{
    model::PackageId,
    process::{self, Output},
    system::Service,
};
use std::{collections::BTreeMap, fs, path::Path, process::Command, time::Duration};
pub(crate) fn valid_unit(unit: &str) -> Result<(), String> {
    if unit.len() > 255
        || !unit.ends_with(".service")
        || unit.starts_with('-')
        || !unit
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Only ordinary, non-template service unit names are supported".into());
    }
    Ok(())
}
fn command(service: &Service) -> Result<Command, String> {
    valid_unit(&service.unit)?;
    let mut command = process::command("/usr/bin/systemctl");
    if service.scope == "user" {
        command.arg("--user");
        for key in ["HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
    } else if service.scope != "system" {
        return Err("Invalid service scope".into());
    }
    Ok(command)
}
fn state(output: Output) -> Result<String, String> {
    if output.timed_out
        || output.truncated
        || output.invalid_utf8
        || !output.stderr.trim().is_empty()
    {
        return Err(format!("Service state unavailable: {}", output.stderr));
    }
    let value = output.stdout.trim();
    // is-enabled returns nonzero for disabled, masked and missing units.
    if !matches!(output.code, Some(0) | Some(1) | Some(3) | Some(4))
        || !matches!(
            value,
            "enabled"
                | "disabled"
                | "enabled-runtime"
                | "linked"
                | "linked-runtime"
                | "alias"
                | "masked"
                | "masked-runtime"
                | "static"
                | "indirect"
                | "generated"
                | "transient"
                | "not-found"
        )
    {
        return Err("Unrecognized service state".into());
    }
    Ok(value.into())
}
pub fn service_state(service: &Service) -> Result<String, String> {
    state(process::run(
        command(service)?.args(["is-enabled", "--", &service.unit]),
        Some(Duration::from_secs(5)),
    )?)
}
fn ownership(id: &PackageId, service: &Service) -> Result<(), String> {
    valid_unit(&service.unit)?;
    if id.provider != "rpm" || id.scope != "system" {
        return Err("Startup changes require a system RPM identity".into());
    }
    let packages = crate::installed()?;
    let package = packages
        .iter()
        .find(|p| p.identity() == id.native)
        .ok_or("Package changed; refresh first")?;
    if crate::providers::protected_service_package(&package.name) {
        return Err("Startup changes for core system components are blocked".into());
    }
    let files = process::run(
        process::command("/usr/bin/rpm").args([
            "-q",
            "--qf",
            "[%{FILENAMES}\\n]",
            "--",
            &id.native,
        ]),
        Some(Duration::from_secs(10)),
    )?
    .query_text()?;
    let path = format!("/usr/lib/systemd/{}/{}", service.scope, service.unit);
    if !files.lines().any(|p| p == path) {
        return Err("Service is not owned by the exact selected package".into());
    }
    if !crate::privileged::trusted_file(Path::new(&path), false) {
        return Err(
            "Service file or its directory is not a trusted root-owned regular file".into(),
        );
    }
    let properties = process::run(
        command(service)?.args([
            "show",
            "--property=LoadState",
            "--property=FragmentPath",
            "--property=DropInPaths",
            "--",
            &service.unit,
        ]),
        Some(Duration::from_secs(5)),
    )?
    .query_text()?;
    let properties: BTreeMap<_, _> = properties
        .lines()
        .filter_map(|l| l.split_once('='))
        .collect();
    if properties.get("LoadState") != Some(&"loaded")
        || properties.get("FragmentPath") != Some(&path.as_str())
        || properties.get("DropInPaths") != Some(&"")
    {
        return Err("Service has overrides, drop-ins, or a different loaded unit; startup changes need a new review".into());
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    simple_install(&text)
}
fn simple_install(text: &str) -> Result<(), String> {
    let mut install = false;
    let text = text.replace("\\\n", "");
    for line in text.lines().map(str::trim) {
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            install = line == "[Install]";
            continue;
        }
        if install {
            if let Some((key, value)) = line.split_once('=') {
                if matches!(key.trim(), "Also" | "Alias") && !value.trim().is_empty() {
                    return Err("This unit changes additional units or aliases; multi-unit startup review is not supported yet".into());
                }
            }
        }
    }
    Ok(())
}
fn expected(enable: bool) -> &'static str {
    if enable {
        "disabled"
    } else {
        "enabled"
    }
}
fn check_state(actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected || !matches!(actual, "enabled" | "disabled") {
        return Err(
            "Service startup state changed or is not eligible. Refresh and review again.".into(),
        );
    }
    Ok(())
}
fn apply(service: &Service, expected_state: &str, enable: bool) -> Result<String, String> {
    check_state(&service_state(service)?, expected_state)?;
    let output = process::run(
        command(service)?.args([
            if enable { "enable" } else { "disable" },
            "--",
            &service.unit,
        ]),
        None,
    )?;
    if !output.success() {
        return Err(format!(
            "Startup change did not complete. State may have changed; refresh before retrying. {}",
            output.stderr
        ));
    }
    let target = if enable { "enabled" } else { "disabled" };
    if service_state(service)? != target {
        return Err("Startup post-check did not match the requested state. Refresh and inspect the unit; no automatic retry was attempted.".into());
    }
    Ok(format!("{}: startup {target}, verified. Its running state was not changed by a start/stop command.",service.unit))
}
pub fn change_startup(id: &PackageId, service: &Service, enable: bool) -> Result<String, String> {
    crate::platform::require_service_control()?;
    ownership(id, service)?;
    check_state(&service_state(service)?, expected(enable))?;
    if service.scope == "system" {
        crate::privileged::invoke(&crate::privileged::Request {
            version: 1,
            action: crate::privileged::Action::ServiceStartup {
                package: id.clone(),
                unit: service.unit.clone(),
                expected_state: expected(enable).into(),
                enable,
            },
        })
    } else {
        apply(service, expected(enable), enable)
    }
}
pub(crate) fn root_change_startup(
    id: &PackageId,
    unit: &str,
    expected_state: &str,
    enable: bool,
) -> Result<String, String> {
    let service = Service {
        unit: unit.into(),
        scope: "system".into(),
    };
    ownership(id, &service)?;
    check_state(expected_state, expected(enable))?;
    apply(&service, expected_state, enable)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_names_reject_options_paths_templates_and_shell_text() {
        for unit in [
            "--help.service",
            "../x.service",
            "a/b.service",
            "foo@bar.service",
            "x;touch.service",
            "x.socket",
            "",
        ] {
            assert!(valid_unit(unit).is_err(), "{unit}");
        }
        assert!(valid_unit("example-worker.service").is_ok());
    }
    #[test]
    fn stale_or_noneligible_states_cannot_change_startup() {
        assert!(check_state("disabled", "disabled").is_ok());
        for state in [
            "enabled",
            "static",
            "masked",
            "alias",
            "indirect",
            "not-found",
        ] {
            assert!(check_state(state, "disabled").is_err());
        }
    }
    #[test]
    fn multiunit_and_alias_install_effects_are_rejected() {
        assert!(simple_install("[Install]\nWantedBy=multi-user.target\n").is_ok());
        for text in [
            "[Install]\nAlso=other.service\n",
            "[Install]\nAlias=other.service\n",
            "[Install]\nAl\\\nso=other.service\n",
        ] {
            assert!(simple_install(text).is_err());
        }
    }
    #[test]
    fn disabled_state_with_nonzero_exit_is_valid_but_errors_are_not() {
        let make = |code, text: &str, error: &str| Output {
            code: Some(code),
            stdout: text.into(),
            stderr: error.into(),
            timed_out: false,
            truncated: false,
            invalid_utf8: false,
        };
        assert_eq!(state(make(1, "disabled\n", "")).unwrap(), "disabled");
        assert!(state(make(1, "disabled", "bus failed")).is_err());
        assert!(state(make(99, "enabled", "")).is_err());
    }
}
