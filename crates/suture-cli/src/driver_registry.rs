use suture_driver::DriverRegistry;
use suture_driver_csv::CsvDriver;
use suture_driver_docx::DocxDriver;
use suture_driver_feed::FeedDriver;
use suture_driver_html::HtmlDriver;
use suture_driver_ical::IcalDriver;
use suture_driver_image::ImageDriver;
use suture_driver_json::JsonDriver;
use suture_driver_markdown::MarkdownDriver;
use suture_driver_otio::OtioDriver;
use suture_driver_pdf::PdfDriver;
use suture_driver_pptx::PptxDriver;
use suture_driver_sql::SqlDriver;
use suture_driver_svg::SvgDriver;
use suture_driver_toml::TomlDriver;
use suture_driver_xlsx::XlsxDriver;
use suture_driver_xml::XmlDriver;
use suture_driver_yaml::YamlDriver;

/// Build a [`DriverRegistry`] with all builtin semantic drivers registered.
pub(crate) fn builtin_registry() -> DriverRegistry {
    let mut registry = DriverRegistry::new();
    registry.register(Box::new(JsonDriver));
    registry.register(Box::new(TomlDriver));
    registry.register(Box::new(CsvDriver));
    registry.register(Box::new(YamlDriver));
    registry.register(Box::new(XmlDriver));
    registry.register(Box::new(MarkdownDriver));
    registry.register(Box::new(ImageDriver));
    registry.register(Box::new(DocxDriver));
    registry.register(Box::new(XlsxDriver));
    registry.register(Box::new(PptxDriver));
    registry.register(Box::new(PdfDriver));
    registry.register(Box::new(SqlDriver));
    registry.register(Box::new(SvgDriver));
    registry.register(Box::new(HtmlDriver));
    registry.register(Box::new(IcalDriver));
    registry.register(Box::new(FeedDriver));
    registry.register(Box::new(OtioDriver));
    registry
}
