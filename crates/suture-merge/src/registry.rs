use suture_driver::DriverRegistry;

pub fn build_registry() -> DriverRegistry {
    #[allow(unused_mut)]
    let mut registry = DriverRegistry::new();
    #[cfg(feature = "json")]
    registry.register(Box::new(suture_driver_json::JsonDriver));
    #[cfg(feature = "yaml")]
    registry.register(Box::new(suture_driver_yaml::YamlDriver));
    #[cfg(feature = "toml")]
    registry.register(Box::new(suture_driver_toml::TomlDriver));
    #[cfg(feature = "csv")]
    registry.register(Box::new(suture_driver_csv::CsvDriver));
    #[cfg(feature = "xml")]
    registry.register(Box::new(suture_driver_xml::XmlDriver));
    #[cfg(feature = "markdown")]
    registry.register(Box::new(suture_driver_markdown::MarkdownDriver));
    #[cfg(feature = "svg")]
    registry.register(Box::new(suture_driver_svg::SvgDriver));
    #[cfg(feature = "html")]
    registry.register(Box::new(suture_driver_html::HtmlDriver));
    #[cfg(feature = "ical")]
    registry.register(Box::new(suture_driver_ical::IcalDriver));
    #[cfg(feature = "feed")]
    registry.register(Box::new(suture_driver_feed::FeedDriver));
    #[cfg(feature = "docx")]
    registry.register(Box::new(suture_driver_docx::DocxDriver));
    #[cfg(feature = "xlsx")]
    registry.register(Box::new(suture_driver_xlsx::XlsxDriver));
    #[cfg(feature = "pptx")]
    registry.register(Box::new(suture_driver_pptx::PptxDriver));
    registry
}
