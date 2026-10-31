//! Installed root-owned helper. Not setuid; authentication is delegated to polkit.
fn main() {
    match linspector_inventory::privileged::helper_main() {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(40);
        }
    }
}
