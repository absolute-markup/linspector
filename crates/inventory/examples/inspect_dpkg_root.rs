//! Read-only validation helper: exports packages from an extracted/mounted dpkg root.
//! It does not chroot, execute image binaries, start services or alter a package DB.
fn main() {
    let root = std::env::args_os()
        .nth(1)
        .expect("Usage: inspect_dpkg_root ROOT");
    let provider = linspector_inventory::providers::inspect_dpkg_root(std::path::Path::new(&root))
        .expect("open dpkg inspection root");
    let inventory = provider.inventory().expect("query dpkg inventory");
    let export = serde_json::json!({
        "source": inventory.source,
        "label": inventory.provider_label,
        "capabilities": inventory.capabilities,
        "warnings": inventory.warnings,
        "entries": inventory.entries,
    });
    println!("{}", serde_json::to_string_pretty(&export).unwrap());
}
