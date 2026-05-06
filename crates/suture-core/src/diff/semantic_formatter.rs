use crate::file_type::FileType;

pub struct SemanticDiffFormatter;

impl SemanticDiffFormatter {
    #[must_use]
    pub fn format(file_path: &str, file_type: FileType, semantic_output: &str) -> String {
        let header = format!("=== {file_path} ===");
        let driver_label = file_type.driver_name();
        if driver_label.is_empty() {
            return format!("{header}\n{semantic_output}");
        }
        format!("{header}  [{driver_label}]\n{semantic_output}")
    }

    #[must_use]
    pub fn format_image_diff(
        file_path: &str,
        old_size: Option<usize>,
        new_size: Option<usize>,
        semantic_output: &str,
    ) -> String {
        let mut lines = vec![format!("=== {file_path} ===  [Image]")];

        match (old_size, new_size) {
            (Some(old), Some(new)) => {
                lines.push(format!(
                    "  File size: {} -> {}",
                    Self::format_bytes(old),
                    Self::format_bytes(new)
                ));
            }
            (None, Some(new)) => {
                lines.push(format!("  File size: {} (new)", Self::format_bytes(new)));
            }
            _ => {}
        }

        if !semantic_output.is_empty() && semantic_output != "no changes" {
            lines.push(String::new());
            for line in semantic_output.lines() {
                lines.push(format!("  {line}"));
            }
        }

        lines.join("\n")
    }

    fn format_bytes(bytes: usize) -> String {
        const KB: usize = 1024;
        const MB: usize = 1024 * KB;
        const GB: usize = 1024 * MB;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{bytes} B")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_basic() {
        let output = SemanticDiffFormatter::format(
            "data.json",
            FileType::Json,
            "  MODIFIED  /name: \"Alice\" -> \"Bob\"",
        );
        assert!(output.starts_with("=== data.json ==="));
        assert!(output.contains("[JSON]"));
        assert!(output.contains("MODIFIED"));
    }

    #[test]
    fn test_format_unknown_type() {
        let output = SemanticDiffFormatter::format("file.xyz", FileType::Unknown, "some diff");
        assert!(output.starts_with("=== file.xyz ==="));
        assert!(!output.contains('['));
        assert!(output.contains("some diff"));
    }

    #[test]
    fn test_format_docx() {
        let output = SemanticDiffFormatter::format(
            "report.docx",
            FileType::Docx,
            "  MODIFIED  /paragraphs/0: Hello -> Goodbye",
        );
        assert!(output.contains("[DOCX]"));
        assert!(output.contains("MODIFIED"));
    }

    #[test]
    fn test_format_image_diff() {
        let output = SemanticDiffFormatter::format_image_diff(
            "photo.png",
            Some(2_516_582),
            Some(838_860),
            "  MODIFIED  /width: 1920 -> 1920\n  MODIFIED  /height: 1080 -> 1080",
        );
        assert!(output.contains("[Image]"));
        assert!(output.contains("File size"));
        assert!(output.contains("MODIFIED"));
    }

    #[test]
    fn test_format_image_new() {
        let output = SemanticDiffFormatter::format_image_diff(
            "new.png",
            None,
            Some(500_000),
            "  ADDED  /width: 100",
        );
        assert!(output.contains("(new)"));
        assert!(output.contains("488.3 KB"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(SemanticDiffFormatter::format_bytes(500), "500 B");
        assert!(SemanticDiffFormatter::format_bytes(2048).contains("KB"));
        assert!(SemanticDiffFormatter::format_bytes(2_400_000).contains("MB"));
        assert!(SemanticDiffFormatter::format_bytes(1_500_000_000).contains("GB"));
    }

    #[test]
    fn test_format_empty_semantic() {
        let output = SemanticDiffFormatter::format("test.json", FileType::Json, "");
        assert!(output.contains("=== test.json ==="));
    }
}
