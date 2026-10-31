//! RPM-owned launcher/service associations. Never execute desktop-file commands.
use super::rpm::RawEntry;
use crate::{
    system::{Application, Service},
    Package,
};
use std::{collections::HashMap, fs};
fn output(program: &str, args: &[&str]) -> Result<String, String> {
    crate::process::large_query(crate::process::command(program).args(args))?.query_text()
}
use super::catalog::desktop_name;
/// Classification is discoverability, not a removal safety assessment.
pub(super) fn category(p: &Package, apps: &[Application]) -> String {
    if !apps.is_empty() {
        "Applications"
    } else if p.name.ends_with("-devel") || p.name.ends_with("-static") {
        "Development"
    } else if p.name.ends_with("-libs") || p.provides.iter().any(|s| s.contains(".so")) {
        "Libraries"
    } else {
        "Packages"
    }
    .into()
}
pub(super) fn raw_inventory() -> Result<(Vec<RawEntry>, Vec<String>), String> {
    let packages = crate::installed()?;
    // One RPM query for file ownership. No shell; filenames are never executed.
    let mut warnings = vec![];
    let ownership = match output(
        "/usr/bin/rpm",
        &["-qa", "--qf", "[%{=NAME}\\t%{FILENAMES}\\n]"],
    ) {
        Ok(s) => s,
        Err(e) => {
            warnings.push(format!("Application/service discovery unavailable: {e}"));
            String::new()
        }
    };
    let mut apps: HashMap<String, Vec<Application>> = HashMap::new();
    let mut services: HashMap<String, Vec<Service>> = HashMap::new();
    for row in ownership.lines() {
        let Some((name, path)) = row.split_once('\t') else {
            continue;
        };
        if path.starts_with("/usr/share/applications/") && path.ends_with(".desktop") {
            if let Ok(text) = fs::read_to_string(path) {
                if let Some(title) = desktop_name(&text) {
                    apps.entry(name.into()).or_default().push(Application {
                        name: title,
                        desktop_file: path.into(),
                    });
                }
            }
        }
        let scope = if path.starts_with("/usr/lib/systemd/user/") {
            "user"
        } else if path.starts_with("/usr/lib/systemd/system/") {
            "system"
        } else {
            continue;
        };
        let unit = path.rsplit('/').next().unwrap_or("");
        if unit.ends_with(".service")
            && !unit.contains('@')
            && fs::symlink_metadata(path)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false)
        {
            services.entry(name.into()).or_default().push(Service {
                unit: unit.into(),
                scope: scope.into(),
            });
        }
    }
    let entries = packages
        .into_iter()
        .map(|p| {
            let applications = apps.get(&p.name).cloned().unwrap_or_default();
            let units = services.get(&p.name).cloned().unwrap_or_default();
            RawEntry {
                category: category(&p, &applications),
                package: p,
                applications,
                services: units,
            }
        })
        .collect();
    Ok((entries, warnings))
}
