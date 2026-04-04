pub(crate) async fn cmd_drivers() -> Result<(), Box<dyn std::error::Error>> {
    let registry = crate::driver_registry::builtin_registry();

    let drivers = registry.list();
    if drivers.is_empty() {
        println!("No semantic drivers available.");
    } else {
        for (name, extensions) in &drivers {
            println!("{} ({})", name, extensions.join(", "));
        }
    }
    Ok(())
}
