fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<serde_json::Value, String> = if args.is_empty() {
        linspector_inventory::installed().map(|p| serde_json::to_value(p).unwrap())
    } else if args.first().map(String::as_str) == Some("--pacman-root") && args.len() == 2 {
        linspector_inventory::providers::inspect_pacman_root(std::path::Path::new(&args[1]))
            .and_then(|p| p.inventory())
            .map(inventory_json)
            .map_err(|e| e.to_string())
    } else if args.first().map(String::as_str) == Some("--snap-fixture") && args.len() == 2 {
        std::fs::read_to_string(&args[1])
            .map_err(|e| e.to_string())
            .and_then(|text| {
                linspector_inventory::providers::inspect_snap_fixture(&text)
                    .map(inventory_json)
                    .map_err(|e| e.to_string())
            })
    } else if args == ["--snap"] {
        linspector_inventory::providers::snap(&linspector_inventory::platform::detect())
            .and_then(|p| p.inventory())
            .map(inventory_json)
            .map_err(|e| e.to_string())
    } else if args == ["--flatpak"] {
        linspector_inventory::providers::flatpak(&linspector_inventory::platform::detect())
            .and_then(|p|p.inventory()).map(|inventory|serde_json::json!({"source":inventory.source,"capabilities":inventory.capabilities,"warnings":inventory.warnings,"entries":inventory.entries})).map_err(|e|e.to_string())
    } else {
        Err("Usage: linspector-inventory [--flatpak | --snap | --pacman-root ROOT | --snap-fixture FILE] (read-only JSON export)".into())
    };
    match result {
        Ok(p) => println!("{}", serde_json::to_string_pretty(&p).unwrap()),
        Err(e) => {
            eprintln!("Inventory failed: {e}");
            std::process::exit(1);
        }
    }
}

fn inventory_json(inventory: linspector_inventory::providers::Inventory) -> serde_json::Value {
    serde_json::json!({"source":inventory.source,"capabilities":inventory.capabilities,"warnings":inventory.warnings,"entries":inventory.entries})
}
