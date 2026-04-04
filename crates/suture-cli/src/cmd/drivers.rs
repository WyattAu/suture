pub(crate) async fn cmd_drivers() -> Result<(), Box<dyn std::error::Error>> {
    use suture_driver::DriverRegistry;
    use suture_driver_csv::CsvDriver;
    use suture_driver_docx::DocxDriver;
    use suture_driver_json::JsonDriver;
    use suture_driver_pptx::PptxDriver;
    use suture_driver_toml::TomlDriver;
    use suture_driver_xlsx::XlsxDriver;
    use suture_driver_xml::XmlDriver;
    use suture_driver_yaml::YamlDriver;

    let mut registry = DriverRegistry::new();
    registry.register(Box::new(JsonDriver));
    registry.register(Box::new(TomlDriver));
    registry.register(Box::new(CsvDriver));
    registry.register(Box::new(YamlDriver));
    registry.register(Box::new(XmlDriver));
    registry.register(Box::new(DocxDriver));
    registry.register(Box::new(XlsxDriver));
    registry.register(Box::new(PptxDriver));

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
