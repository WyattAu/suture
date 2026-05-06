use std::io::{Cursor, Write};

fn zip_to_string(buf: Vec<u8>) -> String {
    unsafe { String::from_utf8_unchecked(buf) }
}

#[must_use]
pub fn simple() -> String {
    make_pptx(&["Title Slide", "Content Slide", "Summary Slide"])
}

#[must_use]
pub fn multi_layout() -> String {
    make_pptx(&[
        "Title",
        "Agenda",
        "Introduction",
        "Market Analysis",
        "Competitive Landscape",
        "Product Overview",
        "Technical Architecture",
        "Demo",
        "Pricing",
        "Customer Testimonials",
        "Roadmap",
        "Q&A",
    ])
}

#[must_use]
pub fn styled() -> String {
    make_pptx(&[
        "Acme Corp Annual Report 2025",
        "Executive Summary",
        "Financial Performance",
        "Product Roadmap",
        "Team Growth",
        "Customer Metrics",
        "Strategic Priorities 2026",
        "Thank You",
    ])
}

#[must_use]
pub fn complex() -> String {
    make_pptx(&[
        "Project Phoenix - Kickoff",
        "Agenda",
        "Project Background",
        "Objectives and KPIs",
        "Team Structure",
        "Timeline Overview",
        "Technical Approach",
        "Risk Assessment",
        "Budget Allocation",
        "Milestone 1: Discovery",
        "Milestone 2: Design",
        "Milestone 3: Implementation",
        "Milestone 4: Testing",
        "Milestone 5: Launch",
        "Appendix: Technical Specs",
    ])
}

fn make_pptx(slide_names: &[impl AsRef<str>]) -> String {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));

        // [Content_Types].xml
        let ct_overrides: String = slide_names
            .iter()
            .enumerate()
            .map(|(i, _)| {
                format!(
                    r#"  <Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#,
                    i + 1
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        zip.start_file(
            "[Content_Types].xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
{ct_overrides}
</Types>"#
            )
            .as_bytes(),
        )
        .unwrap();

        // ppt/_rels/presentation.xml.rels
        let slide_rels: String = slide_names
            .iter()
            .enumerate()
            .map(|(i, _)| {
                format!(
                    r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
                    i + 2,
                    i + 1
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        zip.start_file(
            "ppt/_rels/presentation.xml.rels",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{slide_rels}
</Relationships>"#
            )
            .as_bytes(),
        )
        .unwrap();

        // ppt/presentation.xml with <p:sldIdLst>
        let sld_ids: String = slide_names
            .iter()
            .enumerate()
            .map(|(i, _)| {
                format!(
                    r#"    <p:sldId id="{}" r:id="rId{}"/>"#,
                    256 + i as u32,
                    i + 2
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        zip.start_file(
            "ppt/presentation.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
{sld_ids}
  </p:sldIdLst>
</p:presentation>"#
            )
            .as_bytes(),
        )
        .unwrap();

        // Individual slide files
        for (i, name) in slide_names.iter().enumerate() {
            zip.start_file(
                format!("ppt/slides/slide{}.xml", i + 1),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sp>
    <p:nvSpPr>
      <p:cNvPr id="1" name="{}"/>
    </p:nvSpPr>
  </p:sp>
</p:sld>"#,
                    name.as_ref()
                )
                .as_bytes(),
            )
            .unwrap();
        }

        zip.finish().unwrap();
    }
    zip_to_string(buf)
}

pub fn make_from_slides(slide_names: &[impl AsRef<str>]) -> String {
    make_pptx(slide_names)
}
