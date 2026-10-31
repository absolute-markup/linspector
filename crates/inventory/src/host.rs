//! Read-only system overview. Optional probes fail independently; no privileged queries.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Field {
    pub label: String,
    pub value: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub subtitle: String,
    pub fields: Vec<Field>,
}
impl Section {
    fn new(title: &str, subtitle: &str) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            fields: vec![],
        }
    }
    fn add(&mut self, label: impl Into<String>, value: impl Into<String>) {
        let v = value.into();
        self.fields.push(Field {
            label: label.into(),
            value: if v.trim().is_empty() {
                "Unavailable".into()
            } else {
                v.trim().into()
            },
        });
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub hostname: String,
    pub os: String,
    pub cpu: String,
    pub memory_total: u64,
    pub memory_available: u64,
    pub logical_cpus: usize,
    pub uptime_seconds: u64,
    pub collected_at: u64,
    pub sections: Vec<Section>,
    pub demo: bool,
}
fn read(path: impl AsRef<std::path::Path>) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}
fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}
// Bound optional utilities so System cannot leave the scan waiting on a session bus.
// Readers drain both pipes concurrently, avoiding full-pipe deadlocks.
fn probe(program: &str, args: &[&str]) -> Option<String> {
    use std::process::Stdio;
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = pipe.by_ref().take(4 * 1024 * 1024).read_to_string(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < Duration::from_secs(3) => {
                std::thread::sleep(Duration::from_millis(15))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let text = reader.join().ok()?;
    if status?.success() {
        Some(text.trim().into())
    } else {
        None
    }
}
fn json_probe(program: &str, args: &[&str]) -> Value {
    probe(program, args)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => v.to_string(),
    }
}
fn human_bytes(n: u64) -> String {
    let mut v = n as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut i = 0;
    while v >= 1024. && i < 4 {
        v /= 1024.;
        i += 1;
    }
    format!("{v:.1} {}", units[i])
}
pub fn os_release(input: &str) -> BTreeMap<String, String> {
    input
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let v = v.trim();
            let v = if v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"'))
                    || (v.starts_with('\'') && v.ends_with('\'')))
            {
                &v[1..v.len().saturating_sub(1)]
            } else {
                v
            };
            // No shell expansion: values from os-release remain inert text.
            Some((
                k.trim().into(),
                v.replace("\\\"", "\"").replace("\\\\", "\\"),
            ))
        })
        .collect()
}
pub fn memory_bytes(input: &str) -> BTreeMap<String, u64> {
    input
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            let mut f = v.split_whitespace();
            let n = f.next()?.parse::<u64>().ok()?;
            let unit = f.next().unwrap_or("");
            Some((
                k.into(),
                if unit == "kB" {
                    n.saturating_mul(1024)
                } else {
                    n
                },
            ))
        })
        .collect()
}
/// Select the account by real UID, not an editable USER/LOGNAME environment variable.
pub fn passwd_account(input: &str, uid: &str) -> BTreeMap<String, String> {
    input
        .lines()
        .find_map(|l| {
            let f: Vec<_> = l.split(':').collect();
            if f.len() < 7 || f[2] != uid {
                return None;
            }
            Some(
                [
                    ("Username", f[0]),
                    ("Display name", f[4].split(',').next().unwrap_or("")),
                    ("User ID", f[2]),
                    ("Primary group ID", f[3]),
                    ("Home directory", f[5]),
                    ("Login shell", f[6]),
                ]
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            )
        })
        .unwrap_or_default()
}
fn cpu_fields(v: &Value, rows: &mut Vec<(String, String)>) {
    if let Some(items) = v.as_array() {
        for item in items {
            if let Some(k) = item["field"].as_str() {
                if !item["data"].is_null() {
                    rows.push((k.trim_end_matches(':').into(), text(&item["data"])));
                }
            }
            cpu_fields(&item["children"], rows);
        }
    }
}
fn descendants(v: &Value, rows: &mut Vec<Value>) {
    if let Some(a) = v.as_array() {
        for d in a {
            rows.push(d.clone());
            descendants(&d["children"], rows);
        }
    }
}
pub fn collect() -> Report {
    let mut report = Report {
        collected_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        ..Report::default()
    };
    let release = os_release(&read("/etc/os-release"));
    report.os = release
        .get("PRETTY_NAME")
        .cloned()
        .unwrap_or_else(|| "Linux".into());
    report.hostname = read("/proc/sys/kernel/hostname");
    let mut os = Section::new("Operating system", "The software foundation");
    os.add("Distribution", &report.os);
    os.add("Hostname", &report.hostname);
    os.add("Kernel", read("/proc/sys/kernel/osrelease"));
    os.add("Kernel build", read("/proc/version"));
    os.add(
        "Architecture",
        probe("/usr/bin/uname", &["-m"]).unwrap_or_default(),
    );
    for k in ["VERSION_ID", "VARIANT", "BUILD_ID"] {
        if let Some(v) = release.get(k) {
            os.add(k.replace('_', " "), v);
        }
    }
    os.add(
        "Boot mode",
        if std::path::Path::new("/sys/firmware/efi").exists() {
            "UEFI"
        } else {
            "Legacy BIOS or firmware not exposed"
        },
    );
    os.add(
        "System timezone",
        fs::read_link("/etc/localtime")
            .ok()
            .map(|p| {
                p.to_string_lossy()
                    .trim_start_matches("../usr/share/zoneinfo/")
                    .trim_start_matches("/usr/share/zoneinfo/")
                    .to_string()
            })
            .unwrap_or_else(|| read("/etc/timezone")),
    );
    report.uptime_seconds = read("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.) as u64;
    os.add(
        "Uptime",
        format!(
            "{} days, {} hours, {} minutes",
            report.uptime_seconds / 86400,
            report.uptime_seconds / 3600 % 24,
            report.uptime_seconds / 60 % 60
        ),
    );
    report.sections.push(os);

    let uid = probe("/usr/bin/id", &["-ru"]).unwrap_or_default();
    let nss = if uid.is_empty() {
        String::new()
    } else {
        probe("/usr/bin/getent", &["passwd", &uid]).unwrap_or_else(|| read("/etc/passwd"))
    };
    let mut account = Section::new("Account & session", "The account running Linspector");
    let fields = passwd_account(&nss, &uid);
    for label in [
        "Username",
        "Display name",
        "User ID",
        "Primary group ID",
        "Home directory",
        "Login shell",
    ] {
        account.add(label, fields.get(label).cloned().unwrap_or_default());
    }
    account.add("Groups", probe("/usr/bin/id", &["-Gn"]).unwrap_or_default());
    account.add(
        "Effective user ID",
        probe("/usr/bin/id", &["-u"]).unwrap_or_default(),
    );
    for (label, key) in [
        ("Desktop", "XDG_CURRENT_DESKTOP"),
        ("Desktop session", "DESKTOP_SESSION"),
        ("Session type", "XDG_SESSION_TYPE"),
        ("Session ID", "XDG_SESSION_ID"),
        ("Display server", "WAYLAND_DISPLAY"),
        ("X display", "DISPLAY"),
        ("Session language", "LANG"),
    ] {
        account.add(label, env(key));
    }
    report.sections.push(account);

    let mut hardware = Section::new("Hardware & firmware", "Device identity and platform");
    for (label, file) in [
        ("Manufacturer", "sys_vendor"),
        ("Model", "product_name"),
        ("Product version", "product_version"),
        ("Motherboard", "board_name"),
        ("Board vendor", "board_vendor"),
        ("Firmware vendor", "bios_vendor"),
        ("Firmware version", "bios_version"),
        ("Firmware date", "bios_date"),
    ] {
        hardware.add(label, read(format!("/sys/class/dmi/id/{file}")));
    }
    hardware.add(
        "Virtualization",
        probe("/usr/bin/systemd-detect-virt", &[])
            .unwrap_or_else(|| "None detected or probe unavailable".into()),
    );
    let secure = fs::read_dir("/sys/firmware/efi/efivars")
        .ok()
        .and_then(|d| {
            d.filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().starts_with("SecureBoot-"))
        })
        .and_then(|p| fs::read(p.path()).ok())
        .and_then(|b| b.get(4).copied());
    hardware.add(
        "Secure Boot",
        match secure {
            Some(1) => "Enabled",
            Some(0) => "Disabled",
            _ => "Unavailable",
        },
    );
    report.sections.push(hardware);

    let raw_cpu = json_probe("/usr/bin/lscpu", &["-J"]);
    let mut rows = vec![];
    cpu_fields(&raw_cpu["lscpu"], &mut rows);
    report.cpu = rows
        .iter()
        .find(|(k, _)| k == "Model name")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| {
            read("/proc/cpuinfo")
                .lines()
                .find_map(|l| {
                    l.strip_prefix("model name")
                        .and_then(|s| s.split_once(':'))
                        .map(|(_, v)| v.trim().into())
                })
                .unwrap_or_default()
        });
    report.logical_cpus = rows
        .iter()
        .find(|(k, _)| k == "CPU(s)")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(0)
        });
    let mut cpu = Section::new("Processor", "Topology, clocks, cache and capabilities");
    if rows.is_empty() {
        cpu.add("Model", &report.cpu);
        cpu.add("Logical CPUs", report.logical_cpus.to_string());
    } else {
        for (k, v) in rows {
            cpu.add(k, v);
        }
    }
    report.sections.push(cpu);

    let mem = memory_bytes(&read("/proc/meminfo"));
    report.memory_total = *mem.get("MemTotal").unwrap_or(&0);
    report.memory_available = *mem.get("MemAvailable").unwrap_or(&0);
    let mut memory = Section::new("Memory & swap", "A snapshot of current usage");
    for (label, k) in [
        ("Installed usable RAM", "MemTotal"),
        ("Available RAM", "MemAvailable"),
        ("Used RAM (total − available)", "Used"),
        ("Free RAM", "MemFree"),
        ("Page cache", "Cached"),
        ("Buffers", "Buffers"),
        ("Swap total", "SwapTotal"),
        ("Swap free", "SwapFree"),
        ("Swap cached", "SwapCached"),
    ] {
        memory.add(
            label,
            if k == "Used" {
                human_bytes(report.memory_total.saturating_sub(report.memory_available))
            } else {
                mem.get(k).map(|v| human_bytes(*v)).unwrap_or_default()
            },
        );
    }
    let load = read("/proc/loadavg");
    memory.add(
        "Load averages (1 / 5 / 15 min)",
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" / "),
    );
    report.sections.push(memory);

    let mut graphics = Section::new(
        "Graphics & displays",
        "Graphics devices and connected outputs",
    );
    let pci = probe("/usr/bin/lspci", &["-nn"]).unwrap_or_default();
    for line in pci.lines().filter(|l| {
        l.contains("VGA compatible controller")
            || l.contains("3D controller")
            || l.contains("Display controller")
    }) {
        graphics.add("Graphics adapter", line);
    }
    if let Ok(devices) = fs::read_dir("/sys/class/drm") {
        let mut devices: Vec<_> = devices.filter_map(Result::ok).collect();
        devices.sort_by_key(|d| d.file_name());
        for dev in devices {
            let p = dev.path();
            if read(p.join("status")) == "connected" {
                graphics.add(
                    dev.file_name().to_string_lossy().to_string(),
                    format!(
                        "Connected · advertised modes: {}",
                        read(p.join("modes"))
                            .lines()
                            .take(8)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
            }
        }
    }
    if graphics.fields.is_empty() {
        graphics.add("Graphics information", "Unavailable");
    }
    report.sections.push(graphics);

    let mut storage = Section::new(
        "Storage devices",
        "Physical devices, partitions and mapped volumes",
    );
    let blk = json_probe(
        "/usr/bin/lsblk",
        &[
            "-J",
            "-b",
            "-o",
            "NAME,MODEL,TYPE,SIZE,TRAN,ROTA,RO,FSTYPE,MOUNTPOINTS",
        ],
    );
    let mut devices = vec![];
    descendants(&blk["blockdevices"], &mut devices);
    for d in devices.into_iter().filter(|d| d["type"] != "loop") {
        let details = [
            text(&d["model"]).trim().to_string(),
            text(&d["type"]),
            d["size"].as_u64().map(human_bytes).unwrap_or_default(),
            text(&d["tran"]),
            text(&d["fstype"]),
            text(&d["mountpoints"]),
            if d["ro"] == true {
                "read-only".into()
            } else {
                String::new()
            },
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        storage.add(format!("/dev/{}", text(&d["name"])), details);
    }
    if storage.fields.is_empty() {
        storage.add("Device inventory", "Unavailable");
    }
    report.sections.push(storage);

    let mut network = Section::new("Network interfaces", "Local interface status and addresses");
    let interfaces = json_probe("/usr/bin/ip", &["-j", "address", "show"]);
    if let Some(items) = interfaces.as_array() {
        for iface in items {
            let name = text(&iface["ifname"]);
            network.add(
                format!("{name} · status"),
                format!(
                    "{} · MTU {}",
                    text(&iface["operstate"]),
                    text(&iface["mtu"])
                ),
            );
            network.add(
                format!("{name} · hardware address"),
                text(&iface["address"]),
            );
            if let Some(addrs) = iface["addr_info"].as_array() {
                for addr in addrs {
                    network.add(
                        format!("{name} · {}", text(&addr["family"])),
                        format!(
                            "{}/{} · {}",
                            text(&addr["local"]),
                            text(&addr["prefixlen"]),
                            text(&addr["scope"])
                        ),
                    );
                }
            }
        }
    }
    if network.fields.is_empty() {
        network.add("Interface information", "Unavailable");
    }
    report.sections.push(network);

    let mut power = Section::new(
        "Power & battery",
        "Present power supplies and battery state",
    );
    if let Ok(supplies) = fs::read_dir("/sys/class/power_supply") {
        for supply in supplies.filter_map(Result::ok) {
            let name = supply.file_name().to_string_lossy().to_string();
            let p = supply.path();
            let kind = read(p.join("type"));
            if kind == "Battery" {
                for (label, key) in [
                    ("Status", "status"),
                    ("Charge (%)", "capacity"),
                    ("Health", "health"),
                    ("Manufacturer", "manufacturer"),
                    ("Model", "model_name"),
                    ("Cycle count", "cycle_count"),
                ] {
                    power.add(format!("{name} · {label}"), read(p.join(key)));
                }
            } else if !read(p.join("online")).is_empty() {
                power.add(
                    format!("{name} · {kind}"),
                    if read(p.join("online")) == "1" {
                        "Connected"
                    } else {
                        "Disconnected"
                    },
                );
            }
        }
    }
    if power.fields.is_empty() {
        power.add(
            "Battery",
            "No readable battery or power supply was detected",
        );
    }
    report.sections.push(power);

    let mut security = Section::new(
        "Security & platform state",
        "Reported state, not a security assessment",
    );
    security.add(
        "SELinux",
        match read("/sys/fs/selinux/enforce").as_str() {
            "1" => "Enforcing",
            "0" => "Permissive",
            _ => "Unavailable or not enabled",
        },
    );
    security.add("Kernel lockdown", read("/sys/kernel/security/lockdown"));
    security.add(
        "TPM",
        if std::path::Path::new("/sys/class/tpm/tpm0").exists() {
            "Device present"
        } else {
            "Not exposed"
        },
    );
    report.sections.push(security);
    report
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_values_never_execute_or_expand() {
        let p=os_release("# comment\nPRETTY_NAME=\"Example Linux 44\"\nID=example\nVALUE='$(touch /tmp/should-not-exist)'\n");
        assert_eq!(p["PRETTY_NAME"], "Example Linux 44");
        assert_eq!(p["VALUE"], "$(touch /tmp/should-not-exist)");
    }
    #[test]
    fn memory_uses_binary_kilobytes_and_tolerates_missing_values() {
        let m = memory_bytes("MemTotal: 1024 kB\nMemAvailable: 512 kB\nBad: nope\n");
        assert_eq!(m["MemTotal"], 1048576);
        assert!(!m.contains_key("SwapTotal"));
    }
    #[test]
    fn account_selected_by_real_uid_not_first_account() {
        let p=passwd_account("root:x:0:0:root:/root:/bin/bash\nalex:x:1000:1000:Alex Example,Office:/home/alex:/bin/zsh\n","1000");
        assert_eq!(p["Username"], "alex");
        assert_eq!(p["Display name"], "Alex Example");
        assert_eq!(p["Home directory"], "/home/alex");
        assert!(passwd_account("bad", "1000").is_empty());
    }
    #[test]
    fn hierarchical_cpu_data_is_retained() {
        let j = serde_json::json!([{"field":"Architecture:","data":"x86_64"},{"field":"Caches:","children":[{"field":"L3:","data":"16 MiB"}]}]);
        let mut r = vec![];
        cpu_fields(&j, &mut r);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1], ("L3".into(), "16 MiB".into()));
    }
}
