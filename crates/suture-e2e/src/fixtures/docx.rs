use std::io::{Cursor, Write};

fn zip_to_string(buf: Vec<u8>) -> String {
    unsafe { String::from_utf8_unchecked(buf) }
}

/// Build a DOCX as raw bytes (binary-safe, for use with merge_raw).
fn make_docx_bytes(paragraphs: &[impl AsRef<str>]) -> Vec<u8> {
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

    let mut doc_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n\
         <w:body>\n",
    );
    for p in paragraphs {
        let text = p.as_ref();
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;");
        doc_xml.push_str(&format!("    <w:p><w:r><w:t>{escaped}</w:t></w:r></w:p>\n"));
    }
    doc_xml.push_str("  </w:body>\n</w:document>");

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
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(doc_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    buf
}

#[must_use]
pub fn multi_section_bytes() -> Vec<u8> {
    make_docx_bytes(MULTI_SECTION_PARAGRAPHS)
}

pub fn with_modified_paragraph_bytes(
    paragraphs: &[impl AsRef<str>],
    index: usize,
    new_text: &str,
) -> Vec<u8> {
    let mut modified: Vec<String> = paragraphs.iter().map(|p| p.as_ref().to_owned()).collect();
    if index < modified.len() {
        modified[index] = new_text.to_owned();
    } else {
        modified.push(new_text.to_owned());
    }
    make_docx_bytes(&modified)
}

pub const SIMPLE_PARAGRAPHS: &[&str] = &["This is a simple document with a single paragraph."];

#[must_use]
pub fn simple() -> String {
    make_docx(SIMPLE_PARAGRAPHS)
}

pub const MULTI_SECTION_PARAGRAPHS: &[&str] = &[
    "Quarterly Report Q4 2025",
    "Executive Summary",
    "This quarter saw significant growth across all product lines. Revenue increased by 23% year over year.",
    "Key Metrics",
    "Total revenue reached $4.2 million, exceeding our target of $3.8 million.",
    "Product Development",
    "The engineering team shipped 47 features during Q4.",
    "Customer Success",
    "Net promoter score improved from 62 to 71.",
    "Next Steps",
    "We plan to expand into the European market in Q1 2026.",
];

#[must_use]
pub fn multi_section() -> String {
    make_docx(MULTI_SECTION_PARAGRAPHS)
}

pub const STYLED_PARAGRAPHS: &[&str] = &[
    "BRANDED REPORT: ACME CORPORATION",
    "Confidential",
    "This document contains proprietary information.",
    "Section One: Overview",
    "The following analysis covers fiscal year 2025 performance metrics.",
    "Section Two: Financial Summary",
    "Gross revenue: $12.3 million",
    "Operating expenses: $8.7 million",
    "Net income: $3.6 million",
    "Section Three: Strategic Initiatives",
    "Three major initiatives were launched this year.",
];

#[must_use]
pub fn styled() -> String {
    make_docx(STYLED_PARAGRAPHS)
}

pub const COMPLEX_PARAGRAPHS: &[&str] = &[
    "LEGAL AGREEMENT",
    "PARTIES",
    "This agreement is entered into by and between Acme Corporation and Beta Industries.",
    "ARTICLE 1: SCOPE OF WORK",
    "1.1 The Company shall provide consulting services as described in Exhibit A.",
    "1.2 Services shall be performed in accordance with industry standards.",
    "1.3 Any changes to the scope must be documented in writing.",
    "ARTICLE 2: TERM AND TERMINATION",
    "2.1 This agreement shall commence on January 1, 2026.",
    "2.2 Either party may terminate with thirty days written notice.",
    "2.3 Deliverables shall be transferred to the Client upon termination.",
    "ARTICLE 3: COMPENSATION",
    "3.1 Client shall pay a monthly retainer of $15,000.",
    "3.2 Additional services shall be billed at $200 per hour.",
    "3.3 Invoices are due within thirty days of receipt.",
    "ARTICLE 4: CONFIDENTIALITY",
    "4.1 Both parties agree to maintain strict confidentiality.",
    "4.2 This obligation survives termination for five years.",
    "ARTICLE 5: LIMITATION OF LIABILITY",
    "5.1 Total liability shall not exceed total fees paid.",
    "5.2 Neither party liable for consequential damages.",
    "SIGNATURES",
    "Authorized representative, Acme Corporation",
    "Authorized representative, Beta Industries",
];

#[must_use]
pub fn complex() -> String {
    make_docx(COMPLEX_PARAGRAPHS)
}

#[must_use]
pub fn long_paragraphs() -> Vec<String> {
    let mut paragraphs = Vec::with_capacity(55);
    paragraphs.push("THE COMPREHENSIVE GUIDE TO MODERN SOFTWARE ARCHITECTURE".to_owned());
    paragraphs.push("Chapter 1: Introduction".to_owned());
    paragraphs
        .push("Software architecture is the fundamental organization of a system.".to_owned());
    for i in 1..=10 {
        paragraphs.push(format!("Section 1.{i}: Foundation Concepts"));
        paragraphs.push(format!(
            "This section introduces core architectural concepts including modularity and separation of concerns. Example {i} illustrates these principles."
        ));
    }
    paragraphs.push("Chapter 2: Microservices Architecture".to_owned());
    paragraphs.push(
        "Microservices decompose applications into small, independently deployable services."
            .to_owned(),
    );
    for i in 1..=10 {
        paragraphs.push(format!("Section 2.{i}: Microservice Patterns"));
        paragraphs.push(format!(
            "Pattern {i} demonstrates service boundaries and inter-service communication."
        ));
    }
    paragraphs.push("Chapter 3: Event-Driven Architecture".to_owned());
    paragraphs.push(
        "Event-driven architectures use events for communication between components.".to_owned(),
    );
    for i in 1..=10 {
        paragraphs.push(format!("Section 3.{i}: Event Processing Patterns"));
        paragraphs.push(format!(
            "Section 3.{i} covers event sourcing, CQRS, and saga patterns."
        ));
    }
    paragraphs.push("Chapter 4: Cloud-Native Patterns".to_owned());
    paragraphs
        .push("Cloud-native architectures leverage containerization and orchestration.".to_owned());
    for i in 1..=10 {
        paragraphs.push(format!("Section 4.{i}: Cloud Deployment Strategies"));
        paragraphs.push(format!(
            "Strategy {i} covers progressive delivery and automated rollback."
        ));
    }
    paragraphs.push("Conclusion".to_owned());
    paragraphs.push(
        "Modern software architecture requires continuous learning and adaptation.".to_owned(),
    );
    paragraphs
}

#[must_use]
pub fn long() -> String {
    make_docx(&long_paragraphs())
}

fn make_docx(paragraphs: &[impl AsRef<str>]) -> String {
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

    let mut doc_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n\
         <w:body>\n",
    );
    for p in paragraphs {
        let text = p.as_ref();
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;");
        doc_xml.push_str(&format!("    <w:p><w:r><w:t>{escaped}</w:t></w:r></w:p>\n"));
    }
    doc_xml.push_str("  </w:body>\n</w:document>");

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
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(doc_xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    zip_to_string(buf)
}

pub fn with_modified_paragraph(
    paragraphs: &[impl AsRef<str>],
    index: usize,
    new_text: &str,
) -> String {
    let mut modified: Vec<String> = paragraphs.iter().map(|p| p.as_ref().to_owned()).collect();
    if index < modified.len() {
        modified[index] = new_text.to_owned();
    } else {
        modified.push(new_text.to_owned());
    }
    make_docx(&modified)
}
