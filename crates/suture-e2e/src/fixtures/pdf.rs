pub const SIMPLE_PAGES: &[&str] = &["Hello, World! This is a simple one-page PDF document."];

#[must_use] 
pub fn simple() -> String {
    make_pdf(SIMPLE_PAGES)
}

pub const MULTI_PAGE_PAGES: &[&str] = &[
    "TABLE OF CONTENTS",
    "Chapter 1: Getting Started",
    "This chapter introduces the core concepts and provides setup instructions.",
    "Chapter 2: Configuration",
    "Configuration options are described in detail, with examples for common use cases.",
    "Chapter 3: Advanced Features",
    "Advanced features include custom plugins, webhook integrations, and batch processing.",
    "Chapter 4: Security",
    "Security best practices for authentication, authorization, and data encryption.",
    "Chapter 5: Performance",
    "Performance tuning guidelines covering caching, indexing, and query optimization.",
    "Chapter 6: Deployment",
    "Deployment strategies for containerized and serverless environments.",
    "Appendix A: API Reference",
    "Complete API reference with request and response examples.",
];

#[must_use] 
pub fn multi_page() -> String {
    make_pdf(MULTI_PAGE_PAGES)
}

pub const COMPLEX_PAGES: &[&str] = &[
    "ACME CORPORATION - ANNUAL REPORT 2025",
    "Page 1 of 8 - Confidential",
    "Executive Summary",
    "Page 2 of 8 - Financial Highlights",
    "Revenue Analysis and Growth Projections",
    "Page 3 of 8 - Market Analysis",
    "Competitive Positioning and Market Share",
    "Page 4 of 8 - Product Development",
    "R&D Investment and Innovation Pipeline",
    "Page 5 of 8 - Operations Review",
    "Supply Chain and Manufacturing Efficiency",
    "Page 6 of 8 - Human Resources",
    "Talent Acquisition and Retention Metrics",
    "Page 7 of 8 - Risk Assessment",
    "Market, Regulatory, and Operational Risks",
    "Page 8 of 8 - Outlook",
    "Strategic Priorities for Fiscal Year 2026",
];

#[must_use] 
pub fn complex() -> String {
    make_pdf(COMPLEX_PAGES)
}

fn make_pdf(page_texts: &[impl AsRef<str>]) -> String {
    let mut objects = Vec::new();
    let mut obj_offsets = Vec::new();

    let header = b"%PDF-1.0\n";
    let mut pdf = header.to_vec();

    let num_pages = page_texts.len();
    let catalog_id = 1u32;
    let pages_id = 2u32;
    let font_id = 3u32;
    let first_page_obj_id = 4u32;
    let first_content_id = first_page_obj_id + num_pages as u32;

    for i in 0..num_pages as u32 {
        let page_obj_id = first_page_obj_id + i;
        let content_obj_id = first_content_id + i;

        let text = page_texts[i as usize].as_ref();
        let stream_content = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
        let stream_len = stream_content.len();

        let page_dict = format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 612 792] /Contents {content_obj_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >>"
        );
        let content_obj =
            format!("<< /Length {stream_len} >>\nstream\n{stream_content}\nendstream");

        objects.push((page_obj_id, page_dict));
        objects.push((content_obj_id, content_obj));
    }

    let mut kids = Vec::new();
    for i in 0..num_pages as u32 {
        kids.push(format!("{} 0 R", first_page_obj_id + i));
    }
    let kids_str = kids.join(" ");

    let catalog = format!("<< /Type /Catalog /Pages {pages_id} 0 R >>");
    let pages_dict = format!("<< /Type /Pages /Kids [{kids_str}] /Count {num_pages} >>");
    let font_dict = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned();

    let mut all_objects: Vec<(u32, String)> = vec![
        (catalog_id, catalog),
        (pages_id, pages_dict),
        (font_id, font_dict),
    ];
    all_objects.extend(objects);

    for (id, content) in &all_objects {
        obj_offsets.push(pdf.len());
        let obj_str = format!("{id} 0 obj\n{content}\nendobj\n");
        pdf.extend_from_slice(obj_str.as_bytes());
    }

    let xref_offset = pdf.len();
    let num_objs = all_objects.len() as u32 + 1;

    let xref = format!("xref\n0 {num_objs}\n0000000000 65535 f \n");
    pdf.extend_from_slice(xref.as_bytes());

    for &offset in &obj_offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    let trailer = format!(
        "trailer\n<< /Size {num_objs} /Root {catalog_id} 0 R >>\nstartxref\n{xref_offset}\n%%EOF"
    );
    pdf.extend_from_slice(trailer.as_bytes());

    unsafe { String::from_utf8_unchecked(pdf) }
}

pub fn with_modified_page(page_texts: &[impl AsRef<str>], index: usize, new_text: &str) -> String {
    let mut modified: Vec<String> = page_texts.iter().map(|p| p.as_ref().to_owned()).collect();
    if index < modified.len() {
        modified[index] = new_text.to_owned();
    } else {
        modified.push(new_text.to_owned());
    }
    make_pdf(&modified)
}
