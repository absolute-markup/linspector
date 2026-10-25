fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.is_empty() {
        linspector_inventory::installed()
    } else {
        Err("Usage: linspector-inventory (exports all installed RPM packages as JSON)".into())
    };
    match result {
        Ok(p) => println!("{}", serde_json::to_string_pretty(&p).unwrap()),
        Err(e) => {
            eprintln!("Inventory failed: {e}");
            std::process::exit(1);
        }
    }
}
