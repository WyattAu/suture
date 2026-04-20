#!/usr/bin/env python3
"""
Create modified versions of binary documents for E2E VCS testing.
Each script takes: base_dir (where originals live), output_dir (where modified go)
Creates person-A and person-B modifications for each document type.
"""
import sys
import os
import json
import zipfile
import shutil
import subprocess

def create_modified_docx(base_path, a_path, b_path):
    """DOCX: Person A modifies paragraph 1, Person B modifies paragraph 2"""
    with zipfile.ZipFile(base_path, 'r') as zin:
        base_xml = zin.read('word/document.xml').decode('utf-8')
    
    # Person A: change first text run
    a_xml = base_xml.replace('Hello World', 'Hello from Person A', 1)
    
    # Person B: change second text element or add content
    b_xml = base_xml.replace('Hello World', 'Hello from Person B', 1)
    
    # If no "Hello World", just modify differently
    if a_xml == base_xml:
        a_xml = base_xml.replace('</w:t>', ' (Modified by A)</w:t>', 1)
    if b_xml == base_xml:
        b_xml = base_xml.replace('</w:t>', ' (Modified by B)</w:t>', 1)
    
    with zipfile.ZipFile(base_path, 'r') as zin:
        for item in zin.infolist():
            data = zin.read(item.filename)
            if item.filename == 'word/document.xml':
                data = a_xml.encode('utf-8') if a_path else b_xml.encode('utf-8')
            with open(a_path if a_path else b_path, 'wb') as f:
                pass
            # Copy entire ZIP with modifications
    # Rebuild ZIP for A
    with zipfile.ZipFile(base_path, 'r') as zin:
        with zipfile.ZipFile(a_path, 'w', zipfile.ZIP_DEFLATED) as zout:
            for item in zin.infolist():
                data = zin.read(item.filename)
                if item.filename == 'word/document.xml':
                    data = a_xml.encode('utf-8')
                zout.writestr(item, data)
    # Rebuild ZIP for B
    with zipfile.ZipFile(base_path, 'r') as zin:
        with zipfile.ZipFile(b_path, 'w', zipfile.ZIP_DEFLATED) as zout:
            for item in zin.infolist():
                data = zin.read(item.filename)
                if item.filename == 'word/document.xml':
                    data = b_xml.encode('utf-8')
                zout.writestr(item, data)


def create_modified_xlsx(base_path, a_path, b_path):
    """XLSX: Person A modifies cell B2, Person B modifies cell C2"""
    # Manual ZIP XML editing (no openpyxl available)
    create_modified_xlsx_manual(base_path, a_path, b_path)


def create_modified_xlsx_manual(base_path, a_path, b_path):
    """XLSX fallback: modify cell values via XML in the ZIP"""
    with zipfile.ZipFile(base_path, 'r') as zin:
        all_items = [(item, zin.read(item.filename)) for item in zin.infolist()]
    
    sheet_xml = None
    for item, data in all_items:
        if item.filename == 'xl/worksheets/sheet1.xml':
            sheet_xml = data.decode('utf-8')
            break
    
    if sheet_xml is None:
        print("ERROR: xl/worksheets/sheet1.xml not found in XLSX")
        return
    
    # Person A: modify B2 cell value (replace <v>456</v> in B2 context)
    # B2 looks like: <c r="B2"><v>456</v></c>
    a_xml = sheet_xml.replace(
        '<c r="B2"><v>456</v></c>',
        '<c r="B2" t="inlineStr"><is><t>Modified by Person A</t></is></c>',
        1
    )
    if a_xml == sheet_xml:
        # Fallback: try shared string approach
        a_xml = sheet_xml.replace('r="B2"', 'r="B2" t="inlineStr"', 1)
        a_xml = a_xml.replace('<v>456</v></c>', '<is><t>Modified by Person A</t></is></c>', 1)
    
    # Person B: modify C2 cell value (shared string index 135)
    # C2 looks like: <c r="C2" t="s"><v>135</v></c>
    b_xml = sheet_xml.replace(
        '<c r="C2" t="s"><v>135</v></c>',
        '<c r="C2" t="inlineStr"><is><t>Modified by Person B</t></is></c>',
        1
    )
    if b_xml == sheet_xml:
        # Fallback: try generic approach
        b_xml = sheet_xml.replace('r="C2"', 'r="C2" t="inlineStr"', 1)
    
    # Write A
    with zipfile.ZipFile(a_path, 'w', zipfile.ZIP_DEFLATED) as zout:
        for item, data in all_items:
            d = a_xml.encode('utf-8') if item.filename == 'xl/worksheets/sheet1.xml' else data
            zout.writestr(item, d)
    
    # Write B
    with zipfile.ZipFile(b_path, 'w', zipfile.ZIP_DEFLATED) as zout:
        for item, data in all_items:
            d = b_xml.encode('utf-8') if item.filename == 'xl/worksheets/sheet1.xml' else data
            zout.writestr(item, d)


def create_modified_pptx(base_path, a_path, b_path):
    """PPTX: Person A modifies slide 1 text, Person B adds a note.
    Rebuilds entire ZIP to avoid overlapping entry error."""
    with zipfile.ZipFile(base_path, 'r') as zin:
        all_items = [(item.filename, zin.read(item.filename)) for item in zin.infolist()]
    
    xml_by_name = {}
    for fname, data in all_items:
        xml_by_name[fname] = data
    
    # Modify slide1.xml
    slide1_xml = xml_by_name.get('ppt/slides/slide1.xml', b'').decode('utf-8')
    a_slide1 = slide1_xml.replace('</a:p>', '</a:p><a:p><a:r><a:t>Added by Person A</a:t></a:r></a:p>', 1)
    b_slide1 = slide1_xml.replace('</a:p>', '</a:p><a:p><a:r><a:t>Added by Person B</a:t></a:r></a:p>', 1)
    
    # Write A
    with zipfile.ZipFile(a_path, 'w', zipfile.ZIP_DEFLATED) as zout:
        for fname, data in all_items:
            d = a_slide1.encode('utf-8') if fname == 'ppt/slides/slide1.xml' else data
            zout.writestr(fname, d)
    
    # Write B
    with zipfile.ZipFile(b_path, 'w', zipfile.ZIP_DEFLATED) as zout:
        for fname, data in all_items:
            d = b_slide1.encode('utf-8') if fname == 'ppt/slides/slide1.xml' else data
            zout.writestr(fname, d)


def main():
    if len(sys.argv) < 4:
        print("Usage: create_doc_versions.py <base_dir> <output_dir> <doc_type>")
        sys.exit(1)
    
    base_dir = sys.argv[1]
    output_dir = sys.argv[2]
    doc_type = sys.argv[3]
    
    os.makedirs(output_dir, exist_ok=True)
    
    if doc_type == 'docx':
        base = os.path.join(base_dir, 'sample.docx')
        if not os.path.exists(base):
            print(f"ERROR: {base} not found")
            sys.exit(1)
        create_modified_docx(base, os.path.join(output_dir, 'sample_a.docx'), os.path.join(output_dir, 'sample_b.docx'))
        print("DOCX: created sample_a.docx (modified paragraph) and sample_b.docx (modified paragraph)")
        
    elif doc_type == 'xlsx':
        base = os.path.join(base_dir, 'sample.xlsx')
        if not os.path.exists(base):
            print(f"ERROR: {base} not found")
            sys.exit(1)
        create_modified_xlsx(base, os.path.join(output_dir, 'sample_a.xlsx'), os.path.join(output_dir, 'sample_b.xlsx'))
        print("XLSX: created sample_a.xlsx (modified B2) and sample_b.xlsx (modified C2)")
        
    elif doc_type == 'pptx':
        base = os.path.join(base_dir, 'sample.pptx')
        if not os.path.exists(base):
            print(f"ERROR: {base} not found")
            sys.exit(1)
        create_modified_pptx(base, os.path.join(output_dir, 'sample_a.pptx'), os.path.join(output_dir, 'sample_b.pptx'))
        print("PPTX: created sample_a.pptx (added text) and sample_b.pptx (added text)")
    
    else:
        print(f"Unknown doc type: {doc_type}")
        sys.exit(1)


if __name__ == '__main__':
    main()
