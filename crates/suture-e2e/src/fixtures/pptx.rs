use std::io::{Cursor, Write};

fn zip_to_string(buf: Vec<u8>) -> String {
    unsafe { String::from_utf8_unchecked(buf) }
}

pub fn simple() -> String {
    make_pptx(&["Title Slide", "Content Slide", "Summary Slide"])
}

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
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;

    let mut pres_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\n",
    );
    for name in slide_names {
        let escaped = name.as_ref().replace('"', "&quot;");
        pres_xml.push_str(&format!(
            "  <p:sp name=\"{}\">\n\
             <p:nvSpPr><p:cNvPr id=\"1\" name=\"{}\"/></p:nvSpPr>\n\
             </p:sp>\n",
            escaped, escaped
        ));
    }
    pres_xml.push_str("</p:presentation>");

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        zip.start_file(
            "[Content_Types].xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(content_types.as_bytes()).unwrap();
        zip.start_file(
            "ppt/presentation.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(pres_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    zip_to_string(buf)
}

pub fn make_from_slides(slide_names: &[impl AsRef<str>]) -> String {
    make_pptx(slide_names)
}
