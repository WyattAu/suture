use std::collections::BTreeMap;
use std::io::{Cursor, Write};

pub type CellData = (usize, usize, String);

pub fn simple() -> String {
    make_xlsx(&simple_sheets())
}

pub fn simple_sheets() -> Vec<(&'static str, Vec<CellData>)> {
    vec![(
        "sheet1",
        vec![
            cd(1, 0, "Name"),
            cd(1, 1, "Value"),
            cd(2, 0, "Alpha"),
            cd(2, 1, "100"),
            cd(3, 0, "Beta"),
            cd(3, 1, "200"),
        ],
    )]
}

pub fn multi_sheet() -> String {
    make_xlsx(&multi_sheet_sheets())
}

pub fn multi_sheet_sheets() -> Vec<(&'static str, Vec<CellData>)> {
    vec![
        (
            "Sales",
            vec![
                cd(1, 0, "Month"),
                cd(1, 1, "Revenue"),
                cd(1, 2, "Expenses"),
                cd(1, 3, "Profit"),
                cd(2, 0, "Jan"),
                cd(2, 1, "50000"),
                cd(2, 2, "32000"),
                cd(2, 3, "18000"),
                cd(3, 0, "Feb"),
                cd(3, 1, "55000"),
                cd(3, 2, "33000"),
                cd(3, 3, "22000"),
                cd(4, 0, "Mar"),
                cd(4, 1, "62000"),
                cd(4, 2, "35000"),
                cd(4, 3, "27000"),
                cd(5, 0, "Apr"),
                cd(5, 1, "58000"),
                cd(5, 2, "34000"),
                cd(5, 3, "24000"),
                cd(6, 0, "May"),
                cd(6, 1, "71000"),
                cd(6, 2, "38000"),
                cd(6, 3, "33000"),
                cd(7, 0, "Jun"),
                cd(7, 1, "68000"),
                cd(7, 2, "36000"),
                cd(7, 3, "32000"),
            ],
        ),
        (
            "Employees",
            vec![
                cd(1, 0, "ID"),
                cd(1, 1, "Name"),
                cd(1, 2, "Department"),
                cd(1, 3, "Salary"),
                cd(2, 0, "E001"),
                cd(2, 1, "Alice Chen"),
                cd(2, 2, "Engineering"),
                cd(2, 3, "95000"),
                cd(3, 0, "E002"),
                cd(3, 1, "Bob Martinez"),
                cd(3, 2, "Marketing"),
                cd(3, 3, "78000"),
                cd(4, 0, "E003"),
                cd(4, 1, "Carol Wu"),
                cd(4, 2, "Engineering"),
                cd(4, 3, "102000"),
                cd(5, 0, "E004"),
                cd(5, 1, "David Kim"),
                cd(5, 2, "Sales"),
                cd(5, 3, "88000"),
                cd(6, 0, "E005"),
                cd(6, 1, "Eve Johnson"),
                cd(6, 2, "Engineering"),
                cd(6, 3, "110000"),
            ],
        ),
        (
            "Products",
            vec![
                cd(1, 0, "SKU"),
                cd(1, 1, "Product"),
                cd(1, 2, "Category"),
                cd(1, 3, "Price"),
                cd(1, 4, "Stock"),
                cd(2, 0, "P100"),
                cd(2, 1, "Widget Pro"),
                cd(2, 2, "Hardware"),
                cd(2, 3, "299"),
                cd(2, 4, "450"),
                cd(3, 0, "P101"),
                cd(3, 1, "Gadget Plus"),
                cd(3, 2, "Hardware"),
                cd(3, 3, "199"),
                cd(3, 4, "320"),
                cd(4, 0, "P102"),
                cd(4, 1, "Software Suite"),
                cd(4, 2, "Software"),
                cd(4, 3, "599"),
                cd(4, 4, "999"),
            ],
        ),
    ]
}

pub fn formula_heavy() -> String {
    make_xlsx(&formula_heavy_sheets())
}

pub fn formula_heavy_sheets() -> Vec<(&'static str, Vec<CellData>)> {
    vec![
        (
            "Financials",
            vec![
                cd(1, 0, "Item"),
                cd(1, 1, "Q1"),
                cd(1, 2, "Q2"),
                cd(1, 3, "Q3"),
                cd(1, 4, "Q4"),
                cd(1, 5, "Total"),
                cd(2, 0, "Revenue"),
                cd(2, 1, "100"),
                cd(2, 2, "150"),
                cd(2, 3, "200"),
                cd(2, 4, "250"),
                cd(2, 5, "700"),
                cd(3, 0, "Costs"),
                cd(3, 1, "60"),
                cd(3, 2, "80"),
                cd(3, 3, "90"),
                cd(3, 4, "110"),
                cd(3, 5, "340"),
                cd(4, 0, "Margin"),
                cd(4, 1, "40"),
                cd(4, 2, "70"),
                cd(4, 3, "110"),
                cd(4, 4, "140"),
                cd(4, 5, "360"),
                cd(6, 0, "Average Revenue"),
                cd(6, 1, "175"),
                cd(7, 0, "Total Revenue"),
                cd(7, 1, "700"),
                cd(8, 0, "Min Cost"),
                cd(8, 1, "60"),
                cd(9, 0, "Max Revenue"),
                cd(9, 1, "250"),
            ],
        ),
        (
            "Budgets",
            vec![
                cd(1, 0, "Department"),
                cd(1, 1, "Budget"),
                cd(1, 2, "Spent"),
                cd(1, 3, "Remaining"),
                cd(2, 0, "Engineering"),
                cd(2, 1, "500000"),
                cd(2, 2, "450000"),
                cd(2, 3, "50000"),
                cd(3, 0, "Marketing"),
                cd(3, 1, "200000"),
                cd(3, 2, "195000"),
                cd(3, 3, "5000"),
                cd(4, 0, "Sales"),
                cd(4, 1, "300000"),
                cd(4, 2, "280000"),
                cd(4, 3, "20000"),
                cd(5, 0, "HR"),
                cd(5, 1, "150000"),
                cd(5, 2, "142000"),
                cd(5, 3, "8000"),
                cd(6, 0, "Operations"),
                cd(6, 1, "250000"),
                cd(6, 2, "260000"),
                cd(6, 3, "-10000"),
            ],
        ),
    ]
}

pub fn structured() -> String {
    make_xlsx(&structured_sheets())
}

pub fn structured_sheets() -> Vec<(&'static str, Vec<CellData>)> {
    vec![(
        "Reviews",
        vec![
            cd(1, 0, "Employee Performance Review"),
            cd(2, 0, "Name"),
            cd(2, 1, "Q1"),
            cd(2, 2, "Q2"),
            cd(2, 3, "Q3"),
            cd(2, 4, "Q4"),
            cd(2, 5, "Avg"),
            cd(3, 0, "Alice Chen"),
            cd(3, 1, "4.5"),
            cd(3, 2, "4.8"),
            cd(3, 3, "4.2"),
            cd(3, 4, "4.6"),
            cd(3, 5, "4.5"),
            cd(4, 0, "Bob Martinez"),
            cd(4, 1, "3.8"),
            cd(4, 2, "4.0"),
            cd(4, 3, "3.9"),
            cd(4, 4, "4.1"),
            cd(4, 5, "3.95"),
            cd(6, 0, "Department Summary"),
            cd(7, 0, "Department"),
            cd(7, 1, "Headcount"),
            cd(7, 2, "Avg Rating"),
            cd(8, 0, "Engineering"),
            cd(8, 1, "45"),
            cd(8, 2, "4.3"),
            cd(9, 0, "Marketing"),
            cd(9, 1, "12"),
            cd(9, 2, "4.1"),
            cd(10, 0, "Sales"),
            cd(10, 1, "28"),
            cd(10, 2, "3.9"),
        ],
    )]
}

pub fn wide() -> String {
    make_xlsx(&wide_sheets())
}

pub fn wide_sheets() -> Vec<(&'static str, Vec<CellData>)> {
    let mut cells = Vec::new();
    cells.push(cd(1, 0, "ID"));
    for col in 1..=100 {
        cells.push(cd(1, col, format!("Feature_{col}")));
    }
    for row in 2..=20 {
        cells.push(cd(row, 0, format!("Sample_{row}")));
        for col in 1..=100 {
            let val = (row * 100 + col) % 1000;
            cells.push(cd(row, col, val.to_string()));
        }
    }
    vec![("Dataset", cells)]
}

fn cd(row: usize, col: usize, value: impl Into<String>) -> CellData {
    (row, col, value.into())
}

fn col_to_letter(col: usize) -> String {
    let mut result = String::new();
    let mut n = col;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        n = n / 26;
        if n == 0 {
            break;
        }
        n -= 1;
    }
    result
}

fn zip_to_string(buf: Vec<u8>) -> String {
    unsafe { String::from_utf8_unchecked(buf) }
}

fn make_xlsx(sheets: &[(&str, Vec<CellData>)]) -> String {
    let content_types = r#"<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#;

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        zip.start_file(
            "[Content_Types].xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(content_types.as_bytes()).unwrap();

        for (sheet_name, sheet_cells) in sheets {
            let mut xml = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
                 <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
            );
            let mut rows: BTreeMap<usize, Vec<(usize, &String)>> = BTreeMap::new();
            for &(row, col, ref val) in sheet_cells {
                rows.entry(row).or_default().push((col, val));
            }
            xml.push_str("<sheetData>\n");
            for (row_num, cols) in &rows {
                xml.push_str(&format!("<row r=\"{}\">\n", row_num));
                for (col, val) in cols {
                    let col_letter = col_to_letter(*col);
                    xml.push_str(&format!("<c r=\"{}{}\"><v>{}</v></c>\n", col_letter, row_num, val));
                }
                xml.push_str("</row>\n");
            }
            xml.push_str("</sheetData>\n");
            xml.push_str("</worksheet>");

            let path = format!("xl/worksheets/{}.xml", sheet_name);
            zip.start_file(&path, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    zip_to_string(buf)
}

pub fn with_modified_cell(
    sheets: &[(&str, Vec<CellData>)],
    sheet_idx: usize,
    row: usize,
    col: usize,
    new_val: &str,
) -> String {
    let mut modified_sheets: Vec<(String, Vec<CellData>)> = Vec::new();
    for (i, (name, cells)) in sheets.iter().enumerate() {
        if i == sheet_idx {
            let new_cells: Vec<CellData> = cells
                .iter()
                .map(|&(r, c, ref v)| {
                    if r == row && c == col {
                        (r, c, new_val.to_string())
                    } else {
                        (r, c, v.clone())
                    }
                })
                .collect();
            modified_sheets.push((name.to_string(), new_cells));
        } else {
            modified_sheets.push((name.to_string(), cells.clone()));
        }
    }
    let refs: Vec<(String, Vec<CellData>)> = modified_sheets;
    let slice: Vec<(&str, Vec<CellData>)> =
        refs.iter().map(|(n, c)| (n.as_str(), c.clone())).collect();
    make_xlsx(&slice)
}
