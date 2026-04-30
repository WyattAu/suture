use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Docx,
    Xlsx,
    Pptx,
    Pdf,
    Otio,
    Image,
    Json,
    Yaml,
    Toml,
    Csv,
    Xml,
    Svg,
    Markdown,
    Sql,
    Ical,
    Html,
    Feed,
    Unknown,
}

impl FileType {
    pub fn category(self) -> &'static str {
        match self {
            FileType::Docx | FileType::Xlsx | FileType::Pptx => "document",
            FileType::Otio => "video",
            FileType::Image => "image",
            FileType::Json | FileType::Yaml | FileType::Toml | FileType::Csv | FileType::Xml => {
                "data"
            }
            FileType::Svg => "image",
            FileType::Markdown => "document",
            FileType::Sql => "data",
            FileType::Ical => "data",
            FileType::Pdf => "document",
            FileType::Html => "document",
            FileType::Feed => "data",
            FileType::Unknown => "unknown",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            FileType::Docx | FileType::Markdown | FileType::Pdf => "\u{1F4C4}",
            FileType::Xlsx | FileType::Csv => "\u{1F4CA}",
            FileType::Pptx => "\u{1F3A5}",
            FileType::Otio => "\u{1F3AC}",
            FileType::Image | FileType::Svg => "\u{1F5BC}",
            FileType::Json | FileType::Yaml | FileType::Toml | FileType::Xml | FileType::Sql => {
                "\u{1F4CB}"
            }
            FileType::Ical => "\u{1F4C5}",
            FileType::Html => "\u{1F4C4}",
            FileType::Feed => "\u{1F4E1}",
            FileType::Unknown => "",
        }
    }

    pub fn driver_name(self) -> &'static str {
        match self {
            FileType::Docx => "DOCX",
            FileType::Xlsx => "XLSX",
            FileType::Pptx => "PPTX",
            FileType::Pdf => "PDF",
            FileType::Otio => "OTIO",
            FileType::Image => "Image",
            FileType::Json => "JSON",
            FileType::Yaml => "YAML",
            FileType::Toml => "TOML",
            FileType::Csv => "CSV",
            FileType::Xml => "XML",
            FileType::Svg => "SVG",
            FileType::Markdown => "Markdown",
            FileType::Sql => "SQL",
            FileType::Ical => "ICAL",
            FileType::Html => "HTML",
            FileType::Feed => "FEED",
            FileType::Unknown => "",
        }
    }
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.driver_name())
    }
}

pub fn detect_file_type(path: &Path) -> FileType {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return FileType::Unknown;
    };
    detect_from_extension(ext)
}

pub fn detect_from_extension(ext: &str) -> FileType {
    match ext.to_lowercase().as_str() {
        "docx" => FileType::Docx,
        "xlsx" => FileType::Xlsx,
        "pptx" => FileType::Pptx,
        "pdf" => FileType::Pdf,
        "otio" => FileType::Otio,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "webp" => FileType::Image,
        "json" | "jsonl" => FileType::Json,
        "yaml" | "yml" => FileType::Yaml,
        "toml" => FileType::Toml,
        "csv" | "tsv" => FileType::Csv,
        "xml" | "xsl" | "xsd" | "xhtml" => FileType::Xml,
        "svg" => FileType::Svg,
        "md" | "markdown" => FileType::Markdown,
        "sql" => FileType::Sql,
        "ics" | "ifb" => FileType::Ical,
        "html" | "htm" => FileType::Html,
        "rss" | "atom" => FileType::Feed,
        _ => FileType::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoType {
    Video,
    Document,
    Data,
}

impl RepoType {
    pub fn from_str_value(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "video" => Some(RepoType::Video),
            "document" => Some(RepoType::Document),
            "data" => Some(RepoType::Data),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RepoType::Video => "video",
            RepoType::Document => "document",
            RepoType::Data => "data",
        }
    }

    pub fn matches_file_type(self, ft: FileType) -> bool {
        match self {
            RepoType::Video => ft == FileType::Otio,
            RepoType::Document => matches!(
                ft,
                FileType::Docx
                    | FileType::Xlsx
                    | FileType::Pptx
                    | FileType::Pdf
                    | FileType::Markdown
            ),
            RepoType::Data => matches!(
                ft,
                FileType::Csv
                    | FileType::Json
                    | FileType::Xml
                    | FileType::Yaml
                    | FileType::Toml
                    | FileType::Sql
                    | FileType::Ical
                    | FileType::Feed
            ),
        }
    }
}

pub fn auto_detect_repo_type(dir: &Path) -> Option<RepoType> {
    let mut scores = [
        (RepoType::Video, 0usize),
        (RepoType::Document, 0),
        (RepoType::Data, 0),
    ];

    const MAX_WALK_DEPTH: usize = 10;

    fn walk(dir: &Path, scores: &mut [(RepoType, usize); 3], depth: usize) {
        if depth > MAX_WALK_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // Use symlink_metadata to avoid following symlinks (prevents loops)
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                let name = entry.file_name();
                if name == ".suture" || name == ".git" || name == "target" || name == "node_modules"
                {
                    continue;
                }
                walk(&path, scores, depth + 1);
            } else if meta.is_file() {
                let ft = detect_file_type(&path);
                if ft != FileType::Unknown {
                    for (rt, count) in scores.iter_mut() {
                        if rt.matches_file_type(ft) {
                            *count += 1;
                        }
                    }
                }
            }
        }
    }

    walk(dir, &mut scores, 0);

    scores.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    if scores[0].1 > 0 {
        Some(scores[0].0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_docx() {
        assert_eq!(detect_file_type(Path::new("report.docx")), FileType::Docx);
    }

    #[test]
    fn test_detect_xlsx() {
        assert_eq!(detect_file_type(Path::new("data.xlsx")), FileType::Xlsx);
    }

    #[test]
    fn test_detect_pptx() {
        assert_eq!(detect_file_type(Path::new("slides.pptx")), FileType::Pptx);
    }

    #[test]
    fn test_detect_pdf() {
        assert_eq!(detect_file_type(Path::new("doc.pdf")), FileType::Pdf);
    }

    #[test]
    fn test_detect_otio() {
        assert_eq!(detect_file_type(Path::new("timeline.otio")), FileType::Otio);
    }

    #[test]
    fn test_detect_image_variants() {
        assert_eq!(detect_file_type(Path::new("a.png")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("b.jpg")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("c.jpeg")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("d.gif")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("e.bmp")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("f.tiff")), FileType::Image);
        assert_eq!(detect_file_type(Path::new("g.webp")), FileType::Image);
    }

    #[test]
    fn test_detect_json() {
        assert_eq!(detect_file_type(Path::new("data.json")), FileType::Json);
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(detect_file_type(Path::new("config.yaml")), FileType::Yaml);
        assert_eq!(detect_file_type(Path::new("config.yml")), FileType::Yaml);
    }

    #[test]
    fn test_detect_toml() {
        assert_eq!(detect_file_type(Path::new("Cargo.toml")), FileType::Toml);
    }

    #[test]
    fn test_detect_csv() {
        assert_eq!(detect_file_type(Path::new("data.csv")), FileType::Csv);
    }

    #[test]
    fn test_detect_xml() {
        assert_eq!(detect_file_type(Path::new("data.xml")), FileType::Xml);
    }

    #[test]
    fn test_detect_markdown() {
        assert_eq!(detect_file_type(Path::new("README.md")), FileType::Markdown);
    }

    #[test]
    fn test_detect_sql() {
        assert_eq!(detect_file_type(Path::new("schema.sql")), FileType::Sql);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_file_type(Path::new("video.mp4")), FileType::Unknown);
        assert_eq!(detect_file_type(Path::new("noext")), FileType::Unknown);
    }

    #[test]
    fn test_detect_case_insensitive() {
        assert_eq!(detect_file_type(Path::new("DATA.JSON")), FileType::Json);
        assert_eq!(detect_file_type(Path::new("Photo.PNG")), FileType::Image);
    }

    #[test]
    fn test_file_type_category() {
        assert_eq!(FileType::Docx.category(), "document");
        assert_eq!(FileType::Xlsx.category(), "document");
        assert_eq!(FileType::Otio.category(), "video");
        assert_eq!(FileType::Image.category(), "image");
        assert_eq!(FileType::Json.category(), "data");
        assert_eq!(FileType::Csv.category(), "data");
        assert_eq!(FileType::Markdown.category(), "document");
    }

    #[test]
    fn test_file_type_icon() {
        assert_eq!(FileType::Docx.icon(), "\u{1F4C4}");
        assert_eq!(FileType::Xlsx.icon(), "\u{1F4CA}");
        assert_eq!(FileType::Pptx.icon(), "\u{1F3A5}");
        assert_eq!(FileType::Otio.icon(), "\u{1F3AC}");
        assert_eq!(FileType::Image.icon(), "\u{1F5BC}");
        assert_eq!(FileType::Json.icon(), "\u{1F4CB}");
        assert_eq!(FileType::Unknown.icon(), "");
    }

    #[test]
    fn test_file_type_driver_name() {
        assert_eq!(FileType::Docx.driver_name(), "DOCX");
        assert_eq!(FileType::Json.driver_name(), "JSON");
        assert_eq!(FileType::Unknown.driver_name(), "");
    }

    #[test]
    fn test_repo_type_from_str() {
        assert_eq!(RepoType::from_str_value("video"), Some(RepoType::Video));
        assert_eq!(
            RepoType::from_str_value("document"),
            Some(RepoType::Document)
        );
        assert_eq!(RepoType::from_str_value("data"), Some(RepoType::Data));
        assert_eq!(
            RepoType::from_str_value("Document"),
            Some(RepoType::Document)
        );
        assert_eq!(RepoType::from_str_value("invalid"), None);
    }

    #[test]
    fn test_repo_type_matches_file_type() {
        assert!(RepoType::Video.matches_file_type(FileType::Otio));
        assert!(!RepoType::Video.matches_file_type(FileType::Docx));

        assert!(RepoType::Document.matches_file_type(FileType::Docx));
        assert!(RepoType::Document.matches_file_type(FileType::Xlsx));
        assert!(RepoType::Document.matches_file_type(FileType::Pptx));
        assert!(!RepoType::Document.matches_file_type(FileType::Json));

        assert!(RepoType::Data.matches_file_type(FileType::Csv));
        assert!(RepoType::Data.matches_file_type(FileType::Json));
        assert!(!RepoType::Data.matches_file_type(FileType::Docx));
    }

    #[test]
    fn test_detect_from_extension() {
        assert_eq!(detect_from_extension("json"), FileType::Json);
        assert_eq!(detect_from_extension("JSON"), FileType::Json);
        assert_eq!(detect_from_extension("unknown_ext"), FileType::Unknown);
    }
}
