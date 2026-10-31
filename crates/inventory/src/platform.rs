//! Conservative startup identification and implemented-capability policy.
//! Recognizing a distribution does not imply its management backend is implemented.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, os::unix::fs::PermissionsExt, path::Path};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Facts {
    pub os_release: String,
    pub tools: BTreeSet<String>,
    pub ostree_booted: bool,
    pub transactional: bool,
    pub sandboxed: bool,
    pub systemd_running: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub system_details: bool,
    pub storage: bool,
    pub package_inventory: bool,
    pub flatpak_inventory: bool,
    pub snap_inventory: bool,
    pub reverse_dependencies: bool,
    pub package_removal: bool,
    pub removal_preview: bool,
    pub service_startup: bool,
    pub inventory_reason: String,
    pub removal_reason: String,
    pub service_reason: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub version: String,
    pub family: String,
    pub package_system: String,
    pub management_model: String,
    pub service_manager: String,
    pub detected_sources: Vec<String>,
    pub capabilities: Capabilities,
}
fn family(id: &str, like: &str) -> &'static str {
    fn classify(id: &str) -> Option<&'static str> {
        match id {
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "ol" | "amzn" => Some("dnf-rpm"),
            "opensuse"
            | "suse"
            | "opensuse-tumbleweed"
            | "opensuse-leap"
            | "opensuse-microos"
            | "opensuse-aeon"
            | "aeon"
            | "kalpa"
            | "sles"
            | "sled"
            | "sle-micro" => Some("zypper-rpm"),
            "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary" | "kali" => Some("apt-dpkg"),
            "arch" | "manjaro" | "endeavouros" => Some("pacman"),
            "alpine" => Some("apk"),
            "nixos" => Some("nix"),
            "gentoo" => Some("portage"),
            "void" => Some("xbps"),
            _ => None,
        }
    }
    classify(id)
        .or_else(|| like.split_whitespace().find_map(classify))
        .unwrap_or("unknown")
}
pub fn identify(facts: &Facts) -> Platform {
    let release = crate::host::os_release(&facts.os_release);
    let get = |k: &str| release.get(k).cloned().unwrap_or_default();
    let id = get("ID");
    let family = family(&id, &get("ID_LIKE"));
    let variant = get("VARIANT_ID");
    let model = if family == "nix" {
        "Declarative"
    } else if facts.ostree_booted
        || [
            "silverblue",
            "kinoite",
            "sericea",
            "onyx",
            "sway-atomic",
            "budgie-atomic",
            "cosmic-atomic",
        ]
        .contains(&variant.as_str())
    {
        "Image-based"
    } else if facts.transactional
        || [
            "opensuse-microos",
            "opensuse-aeon",
            "aeon",
            "kalpa",
            "sle-micro",
        ]
        .contains(&id.as_str())
    {
        "Transactional"
    } else if family != "unknown" {
        "Mutable"
    } else {
        "Unknown"
    };
    let has = |tool: &str| facts.tools.contains(tool);
    let package_system = match family {
        "dnf-rpm" => {
            if has("dnf5") {
                "RPM / DNF5"
            } else if has("dnf") {
                "RPM / DNF"
            } else {
                "RPM"
            }
        }
        "zypper-rpm" => "RPM / Zypper",
        "apt-dpkg" => "dpkg / APT",
        "pacman" => "pacman",
        "apk" => "apk",
        "nix" => "Nix",
        "portage" => "Portage",
        "xbps" => "XBPS",
        _ => "Unknown",
    };
    let rpm_family = matches!(family, "dnf-rpm" | "zypper-rpm");
    let apt_family = family == "apt-dpkg";
    let inventory = ((rpm_family && has("rpm"))
        || (apt_family && has("dpkg-query"))
        || (family == "pacman" && has("pacman")))
        && !facts.sandboxed;
    // Preserve the existing, verified Fedora 44 scope. New versions/derivatives
    // receive inspection before management; ID_LIKE never authorizes mutations.
    let verified =
        id == "fedora" && get("VERSION_ID") == "44" && model == "Mutable" && !facts.sandboxed;
    let preview = (verified && has("dnf5")
        || (family == "zypper-rpm" && model == "Mutable" && has("zypper")))
        && inventory;
    let removal = verified && inventory && has("dnf5") && has("pkexec") && has("linspector-helper");
    let services = verified
        && inventory
        && facts.systemd_running
        && has("systemctl")
        && has("pkexec")
        && has("linspector-helper");
    let inventory_reason = if facts.sandboxed {
        "Host package discovery is unavailable in a detected sandbox.".into()
    } else if inventory {
        String::new()
    } else if rpm_family {
        "The RPM query tool is unavailable. System and Storage remain available.".into()
    } else if apt_family {
        "The dpkg-query tool is unavailable. System and Storage remain available.".into()
    } else if family == "pacman" {
        "Pacman was not detected. Explicit root inspection remains available for a copied/mounted ALPM database.".into()
    } else {
        format!("{} was detected; its package inventory integration is not implemented yet. System and Storage remain available.",package_system)
    };
    let reason = if facts.sandboxed {
        "Package-changing actions are unavailable in a sandbox."
    } else if apt_family {
        "APT/dpkg inspection is read-only in this milestone; package changes are not implemented."
    } else if family == "zypper-rpm" {
        "Zypper removal previews are read-only; authenticated execution is not implemented yet."
    } else if family == "pacman" {
        "Pacman inspection is read-only; native package changes are not implemented."
    } else if model != "Mutable" {
        "This system needs a dedicated image-based, transactional or declarative management workflow."
    } else if !verified {
        "Package-changing actions are currently implemented only for mutable Fedora 44 with the trusted helper."
    } else if !has("linspector-helper") {
        "The trusted management helper/policy is not installed. Removal previews remain available; package and startup changes are unavailable."
    } else {
        "Required management tools are missing."
    };
    let service_manager = if facts.systemd_running && has("systemctl") {
        "systemd"
    } else if has("rc-service") {
        "OpenRC (tool detected)"
    } else if has("sv") {
        "runit (tool detected)"
    } else {
        "Unknown"
    };
    let mut detected_sources = vec![];
    for (tool, label) in [
        ("flatpak", "Flatpak detected · read-only inventory"),
        ("snap", "Snap detected · read-only inventory"),
    ] {
        if has(tool) {
            detected_sources.push(label.into());
        }
    }
    Platform {
        id: id.clone(),
        name: {
            let n = get("PRETTY_NAME");
            if n.is_empty() {
                if id.is_empty() {
                    "Unidentified Linux".into()
                } else {
                    id
                }
            } else {
                n
            }
        },
        version: get("VERSION_ID"),
        family: family.into(),
        package_system: package_system.into(),
        management_model: model.into(),
        service_manager: service_manager.into(),
        detected_sources,
        capabilities: Capabilities {
            system_details: true,
            storage: true,
            package_inventory: inventory,
            flatpak_inventory: has("flatpak") && !facts.sandboxed,
            snap_inventory: has("snap") && !facts.sandboxed,
            reverse_dependencies: inventory,
            package_removal: removal,
            removal_preview: preview,
            service_startup: services,
            inventory_reason,
            removal_reason: if removal {
                String::new()
            } else {
                reason.into()
            },
            service_reason: if services {
                String::new()
            } else {
                if !facts.systemd_running {
                    "No supported running systemd environment was detected.".into()
                } else {
                    reason.into()
                }
            },
        },
    }
}
fn executable(path: &str) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
pub fn detect() -> Platform {
    // os-release files are alternatives, not merged. /etc takes precedence.
    let release = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    let mut tools = BTreeSet::new();
    for tool in [
        "rpm",
        "dnf5",
        "dnf",
        "zypper",
        "dpkg-query",
        "apt-get",
        "pacman",
        "apk",
        "nix",
        "emerge",
        "xbps-query",
        "flatpak",
        "snap",
        "systemctl",
        "rc-service",
        "sv",
        "pkexec",
    ] {
        if ["/usr/bin", "/usr/sbin", "/bin", "/sbin"]
            .iter()
            .any(|dir| executable(&format!("{dir}/{tool}")))
        {
            tools.insert(tool.into());
        }
    }
    if crate::privileged::available() {
        tools.insert("linspector-helper".into());
    }
    identify(&Facts {
        os_release: release,
        tools,
        ostree_booted: Path::new("/run/ostree-booted").exists(),
        transactional: Path::new("/etc/transactional-update.conf").exists(),
        sandboxed: Path::new("/.flatpak-info").exists()
            || Path::new("/run/.containerenv").exists()
            || Path::new("/.dockerenv").exists()
            || std::env::var_os("SNAP").is_some(),
        systemd_running: Path::new("/run/systemd/system").is_dir(),
    })
}
pub fn require_removal_preview() -> Result<(), String> {
    let p = detect();
    if p.capabilities.removal_preview {
        Ok(())
    } else {
        Err(p.capabilities.removal_reason)
    }
}
pub fn require_removal() -> Result<(), String> {
    let p = detect();
    if p.capabilities.package_removal {
        Ok(())
    } else {
        Err(p.capabilities.removal_reason)
    }
}
pub fn require_service_control() -> Result<(), String> {
    let p = detect();
    if p.capabilities.service_startup {
        Ok(())
    } else {
        Err(p.capabilities.service_reason)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn facts(id: &str, like: &str) -> Facts {
        Facts {
            os_release: format!("ID={id}\nID_LIKE=\"{like}\"\nVERSION_ID=44\n"),
            tools: ["rpm", "dnf5", "pkexec", "systemctl", "linspector-helper"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            systemd_running: true,
            ..Default::default()
        }
    }
    #[test]
    fn verified_fedora_and_new_releases_are_distinct() {
        let f = facts("fedora", "");
        assert!(identify(&f).capabilities.package_removal);
        let mut newer = f;
        newer.os_release = newer.os_release.replace("44", "45");
        let p = identify(&newer);
        assert!(p.capabilities.package_inventory);
        assert!(!p.capabilities.package_removal);
    }
    #[test]
    fn other_families_never_inherit_dnf_from_installed_tools() {
        for (id, like, expected) in [
            ("ubuntu", "debian", "apt-dpkg"),
            ("linuxmint", "ubuntu debian", "apt-dpkg"),
            ("arch", "", "pacman"),
            ("opensuse-tumbleweed", "suse", "zypper-rpm"),
            ("alpine", "", "apk"),
            ("nixos", "", "nix"),
            ("void", "", "xbps"),
            ("gentoo", "", "portage"),
        ] {
            let p = identify(&facts(id, like));
            assert_eq!(p.family, expected);
            assert!(!p.capabilities.package_removal);
            assert!(!p.capabilities.service_startup);
        }
    }
    #[test]
    fn derivative_family_is_inventory_evidence_not_mutation_authority() {
        let p = identify(&facts("custom", "fedora rhel"));
        assert!(p.capabilities.package_inventory);
        assert!(!p.capabilities.package_removal);
        let p = identify(&facts("custom", "ubuntu debian"));
        assert_eq!(p.family, "apt-dpkg");
        assert!(!p.capabilities.package_inventory);
    }
    #[test]
    fn immutable_and_sandboxed_fedora_block_mutations() {
        let mut f = facts("fedora", "");
        f.ostree_booted = true;
        let p = identify(&f);
        assert_eq!(p.management_model, "Image-based");
        assert!(!p.capabilities.package_removal);
        f.ostree_booted = false;
        f.os_release.push_str("VARIANT_ID=silverblue\n");
        assert!(!identify(&f).capabilities.package_removal);
        f.sandboxed = true;
        assert!(!identify(&f).capabilities.package_inventory);
    }
    #[test]
    fn missing_metadata_or_tools_does_not_guess_a_package_system() {
        let p = identify(&Facts::default());
        assert_eq!(p.family, "unknown");
        assert!(!p.capabilities.package_inventory);
        assert!(p.capabilities.system_details);
        let mut f = facts("fedora", "");
        f.tools.clear();
        assert!(!identify(&f).capabilities.package_inventory);
        assert!(!identify(&f).capabilities.package_removal);
    }
    #[test]
    fn transactional_and_non_systemd_capabilities_are_explicit() {
        let p = identify(&facts("opensuse-microos", "suse"));
        assert_eq!(p.management_model, "Transactional");
        assert!(!p.capabilities.package_removal);
        let mut f = facts("fedora", "");
        f.systemd_running = false;
        f.tools.insert("rc-service".into());
        let p = identify(&f);
        assert_eq!(p.service_manager, "OpenRC (tool detected)");
        assert!(!p.capabilities.service_startup);
    }
}

#[cfg(test)]
mod management_tests {
    use super::*;
    #[test]
    fn missing_helper_preserves_previews_without_enabling_mutations() {
        let facts = Facts {
            os_release: "ID=fedora\nVERSION_ID=44\n".into(),
            tools: ["rpm", "dnf5", "pkexec", "systemctl"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            systemd_running: true,
            ..Default::default()
        };
        let platform = identify(&facts);
        assert!(platform.capabilities.removal_preview);
        assert!(!platform.capabilities.package_removal);
        assert!(!platform.capabilities.service_startup);
        assert!(platform.capabilities.removal_reason.contains("helper"));
        assert!(
            crate::providers::select(&platform)
                .unwrap()
                .capabilities()
                .removal_preview
        );
    }
}
