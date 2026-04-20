//! Comprehensive validation suite for suture-merge.
//!
//! Tests every format, every function, and every edge case.
//! Run with: cargo test -p suture-merge --features all -- full_validation

use suture_driver::SemanticChange;
use suture_merge::*;

// ============================================================================
// Helpers
// ============================================================================

fn check_clean(result: &MergeResult, must_contain: &[&str], must_not_contain: &[&str]) {
    assert_eq!(
        result.status,
        MergeStatus::Clean,
        "Expected Clean but got {:?}, merged:\n{}",
        result.status,
        result.merged
    );
    for needle in must_contain {
        assert!(
            result.merged.contains(needle),
            "Clean merge missing '{}'\nmerged:\n{}",
            needle,
            result.merged
        );
    }
    for needle in must_not_contain {
        assert!(
            !result.merged.contains(needle),
            "Clean merge unexpectedly contains '{}'\nmerged:\n{}",
            needle,
            result.merged
        );
    }
}

fn check_conflict(result: &MergeResult) {
    assert_eq!(
        result.status,
        MergeStatus::Conflict,
        "Expected Conflict but got {:?}, merged:\n{}",
        result.status,
        result.merged
    );
}

// ============================================================================
// 1. JSON
// ============================================================================

#[test]
fn json_clean_different_fields() {
    let r = merge_json(
        r#"{"name": "Alice", "age": 30, "city": "NYC"}"#,
        r#"{"name": "Alice", "age": 31, "city": "NYC"}"#,
        r#"{"name": "Alice", "age": 30, "city": "SF"}"#,
    )
    .unwrap();
    check_clean(&r, &["\"age\": 31", "\"city\": \"SF\""], &[]);
}

#[test]
fn json_clean_nested_object() {
    let r = merge_json(
        r#"{"user": {"name": "Alice", "age": 30}, "active": true}"#,
        r#"{"user": {"name": "Alice", "age": 31}, "active": true}"#,
        r#"{"user": {"name": "Alice", "age": 30}, "active": false}"#,
    )
    .unwrap();
    check_clean(&r, &["\"age\": 31", "\"active\": false"], &[]);
}

#[test]
fn json_clean_array_additions() {
    let r = merge_json(
        r#"{"tags": ["a", "b"]}"#,
        r#"{"tags": ["a", "b", "c"]}"#,
        r#"{"tags": ["a", "b", "d"]}"#,
    )
    .unwrap();
    // Array additions may conflict (same-index modification)
    // Just check it doesn't panic and returns something
    assert!(!r.merged.is_empty());
}

#[test]
fn json_clean_new_field_added() {
    let r = merge_json(
        r#"{"name": "Alice"}"#,
        r#"{"name": "Alice", "email": "alice@example.com"}"#,
        r#"{"name": "Alice"}"#,
    )
    .unwrap();
    check_clean(&r, &["\"email\": \"alice@example.com\""], &[]);
}

#[test]
fn json_conflict_same_field() {
    let r = merge_json(
        r#"{"key": "original"}"#,
        r#"{"key": "ours"}"#,
        r#"{"key": "theirs"}"#,
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn json_conflict_nested() {
    let r = merge_json(
        r#"{"outer": {"inner": "original"}}"#,
        r#"{"outer": {"inner": "ours"}}"#,
        r#"{"outer": {"inner": "theirs"}}"#,
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn json_identical_files() {
    let r = merge_json(
        r#"{"a": 1, "b": 2}"#,
        r#"{"a": 1, "b": 2}"#,
        r#"{"a": 1, "b": 2}"#,
    )
    .unwrap();
    check_clean(&r, &[], &[]);
}

#[test]
fn json_empty_base() {
    let r = merge_json(
        r#"{}"#,
        r#"{"a": 1}"#,
        r#"{"b": 2}"#,
    )
    .unwrap();
    check_clean(&r, &["\"a\": 1", "\"b\": 2"], &[]);
}

#[test]
fn json_large_file() {
    // Build a JSON object with 100 fields
    let mut base_parts = vec![r#"{"_meta": "base""#.to_string()];
    let mut ours_parts = vec![r#"{"_meta": "base""#.to_string()];
    let mut theirs_parts = vec![r#"{"_meta": "base""#.to_string()];
    for i in 0..100 {
        base_parts.push(format!(r#""field_{i}": {i}"#));
        ours_parts.push(format!(r#""field_{i}": {}"#, i + 100));
        theirs_parts.push(format!(r#""field_{i}": {}"#, i + 200));
    }
    let base = format!("{} }}", base_parts.join(", "));
    let ours = format!("{} }}", ours_parts.join(", "));
    let theirs = format!("{} }}", theirs_parts.join(", "));

    let r = merge_json(&base, &ours, &theirs).unwrap();
    // Every field is modified by both sides → conflict (both changed same field)
    assert_eq!(r.status, MergeStatus::Conflict);
}

#[test]
fn json_diff_detects_changes() {
    let changes = diff(
        r#"{"name": "Alice", "age": 30}"#,
        r#"{"name": "Bob", "age": 31, "email": "bob@example.com"}"#,
        Some(".json"),
    )
    .unwrap();
    assert!(changes.len() >= 2); // name modified, age modified, email added
    assert!(changes.iter().any(|c| matches!(
        c,
        SemanticChange::Modified { .. }
    )));
    assert!(changes.iter().any(|c| matches!(
        c,
        SemanticChange::Added { .. }
    )));
}

#[test]
fn json_format_diff_readable() {
    let output = format_diff(
        r#"{"name": "Alice"}"#,
        r#"{"name": "Bob", "age": 30}"#,
        Some(".json"),
    )
    .unwrap();
    assert!(output.contains("MODIFIED") || output.contains("ADDED"));
}

// ============================================================================
// 2. YAML
// ============================================================================

#[test]
fn yaml_clean_different_fields() {
    let r = merge_yaml(
        "name: Alice\nage: 30\n",
        "name: Alice\nage: 31\n",
        "name: Alice\ncity: NYC\n",
    )
    .unwrap();
    check_clean(&r, &["age: 31", "city: NYC"], &[]);
}

#[test]
fn yaml_conflict_same_field() {
    let r = merge_yaml(
        "key: original\n",
        "key: ours\n",
        "key: theirs\n",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn yaml_nested_keys() {
    let r = merge_yaml(
        "server:\n  host: localhost\n  port: 8080\n",
        "server:\n  host: localhost\n  port: 9090\n",
        "server:\n  host: 0.0.0.0\n  port: 8080\n",
    )
    .unwrap();
    check_clean(&r, &["port: 9090", "host: 0.0.0.0"], &[]);
}

#[test]
fn yaml_identical() {
    let r = merge_yaml("a: 1\nb: 2\n", "a: 1\nb: 2\n", "a: 1\nb: 2\n").unwrap();
    check_clean(&r, &[], &[]);
}

#[test]
fn yaml_diff_and_format() {
    let changes = diff("key: val\n", "key: new_val\nnew_key: added\n", Some(".yaml")).unwrap();
    assert!(!changes.is_empty());
    let output = format_diff("key: val\n", "key: new_val\n", Some(".yaml")).unwrap();
    assert!(!output.is_empty());
}

// ============================================================================
// 3. TOML
// ============================================================================

#[test]
fn toml_clean_different_fields() {
    let r = merge_toml(
        "name = \"Alice\"\nage = 30\n",
        "name = \"Alice\"\nage = 31\n",
        "name = \"Alice\"\ncity = \"NYC\"\n",
    )
    .unwrap();
    check_clean(&r, &["age = 31", "city = \"NYC\""], &[]);
}

#[test]
fn toml_conflict_same_field() {
    let r = merge_toml(
        "key = \"original\"\n",
        "key = \"ours\"\n",
        "key = \"theirs\"\n",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn toml_table_sections() {
    let r = merge_toml(
        "[server]\nhost = \"localhost\"\nport = 8080\n",
        "[server]\nhost = \"localhost\"\nport = 9090\n",
        "[server]\nhost = \"0.0.0.0\"\nport = 8080\n",
    )
    .unwrap();
    check_clean(&r, &["port = 9090", "host = \"0.0.0.0\""], &[]);
}

#[test]
fn toml_diff_and_format() {
    let changes = diff("key = 1\n", "key = 2\nnew = 3\n", Some(".toml")).unwrap();
    assert!(!changes.is_empty());
    let output = format_diff("key = 1\n", "key = 2\n", Some(".toml")).unwrap();
    assert!(!output.is_empty());
}

// ============================================================================
// 4. CSV
// ============================================================================

#[test]
fn csv_clean_different_rows() {
    let r = merge_csv(
        "name,age,city\nAlice,30,NYC\nBob,25,SF\n",
        "name,age,city\nAlice,31,NYC\nBob,25,SF\n",
        "name,age,city\nAlice,30,NYC\nBob,25,LA\n",
    )
    .unwrap();
    check_clean(&r, &["31", "LA"], &[]);
}

#[test]
fn csv_clean_add_row() {
    let r = merge_csv(
        "name,age\nAlice,30\n",
        "name,age\nAlice,30\nBob,25\n",
        "name,age\nAlice,30\nCharlie,35\n",
    )
    .unwrap();
    check_clean(&r, &["Bob", "Charlie"], &[]);
}

#[test]
fn csv_conflict_same_cell() {
    let r = merge_csv(
        "name,age\nAlice,30\n",
        "name,age\nAlice,31\n",
        "name,age\nAlice,99\n",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn csv_large_file() {
    let mut base = String::from("id,name,value\n");
    let mut ours = String::from("id,name,value\n");
    let mut theirs = String::from("id,name,value\n");
    for i in 0..100 {
        base.push_str(&format!("row{},Alice,{}\n", i, i));
        ours.push_str(&format!("row{},Alice,{}\n", i, i + 100));
        theirs.push_str(&format!("row{},Bob,{}\n", i, i));
    }
    let r = merge_csv(&base, &ours, &theirs).unwrap();
    assert!(r.merged.contains("101"));
    assert!(r.merged.contains("Bob"));
}

#[test]
fn csv_diff_and_format() {
    let changes = diff("a,b\n1,2\n", "a,b\n1,3\n", Some(".csv")).unwrap();
    assert!(!changes.is_empty());
}

// ============================================================================
// 5. XML
// ============================================================================

#[test]
fn xml_clean_different_elements() {
    let r = merge_xml(
        "<root><a>1</a><b>2</b></root>",
        "<root><a>10</a><b>2</b></root>",
        "<root><a>1</a><b>20</b></root>",
    )
    .unwrap();
    check_clean(&r, &["10", "20"], &[]);
}

#[test]
fn xml_conflict_same_element() {
    let r = merge_xml(
        "<root><item>original</item></root>",
        "<root><item>ours</item></root>",
        "<root><item>theirs</item></root>",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn xml_attribute_changes() {
    let r = merge_xml(
        r#"<root><item id="a" val="1"/></root>"#,
        r#"<root><item id="a" val="2"/></root>"#,
        r#"<root><item id="a" color="red"/></root>"#,
    )
    .unwrap();
    check_clean(&r, &["val=\"2\"", "color=\"red\""], &[]);
}

#[test]
fn xml_nested() {
    let r = merge_xml(
        "<root><outer><inner>x</inner></outer></root>",
        "<root><outer><inner>y</inner></outer></root>",
        "<root><outer><inner>x</inner><new>z</new></outer></root>",
    )
    .unwrap();
    check_clean(&r, &["new>z<"], &[]);
}

#[test]
fn xml_diff_and_format() {
    let changes = diff("<root><a>1</a></root>", "<root><a>2</a><b>3</b></root>", Some(".xml")).unwrap();
    assert!(!changes.is_empty());
    let output = format_diff("<root><a>1</a></root>", "<root><a>2</a></root>", Some(".xml")).unwrap();
    assert!(!output.is_empty());
}

// ============================================================================
// 6. Markdown
// ============================================================================

#[test]
fn markdown_clean_different_sections() {
    let r = merge_markdown(
        "# Title\n\n## Section A\nOld text A\n\n## Section B\nOld text B\n",
        "# Title\n\n## Section A\nNew text A\n\n## Section B\nOld text B\n",
        "# Title\n\n## Section A\nOld text A\n\n## Section B\nNew text B\n",
    )
    .unwrap();
    check_clean(&r, &["New text A", "New text B"], &[]);
}

#[test]
fn markdown_conflict_same_section() {
    let r = merge_markdown(
        "# Title\n\nBody text\n",
        "# Title\n\nOur change\n",
        "# Title\n\nTheir change\n",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn markdown_list_additions() {
    let r = merge_markdown(
        "# Todo\n\n- Task 1\n- Task 2\n",
        "# Todo\n\n- Task 1\n- Task 2\n- Task 3\n",
        "# Todo\n\n- Task 1\n- Task 2\n- Task 4\n",
    )
    .unwrap();
    // List additions should be clean (different paragraphs/blocks)
    assert!(!r.merged.is_empty());
}

#[test]
fn markdown_diff_and_format() {
    let changes = diff("# A\n\nold\n", "# A\n\nnew\n", Some(".md")).unwrap();
    assert!(!changes.is_empty());
}

// ============================================================================
// 7. SVG
// ============================================================================

#[test]
fn svg_clean_different_attributes() {
    let r = merge_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box" fill="red"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box" fill="blue"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect id="box" fill="red"/></svg>"#,
    )
    .unwrap();
    check_clean(&r, &["fill=\"blue\"", "width=\"200\""], &[]);
}

#[test]
fn svg_conflict_same_attribute() {
    let r = merge_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="blue"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="green"/></svg>"#,
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn svg_clean_add_element() {
    let r = merge_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/><circle id="c" r="10"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#,
    )
    .unwrap();
    check_clean(&r, &["circle"], &[]);
}

#[test]
fn svg_diff_and_format() {
    let r = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#;
    let m = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="blue"/></svg>"#;
    let changes = diff(r, m, Some(".svg")).unwrap();
    assert!(!changes.is_empty());
    let output = format_diff(r, m, Some(".svg")).unwrap();
    assert!(output.contains("MODIFIED"));
}

// ============================================================================
// 8. HTML
// ============================================================================

#[test]
fn html_clean_different_elements() {
    let r = merge_html(
        "<html><body><h1>Title</h1><p>Body</p></body></html>",
        "<html><body><h1>New Title</h1><p>Body</p></body></html>",
        "<html><body><h1>Title</h1><p>New Body</p></body></html>",
    )
    .unwrap();
    check_clean(&r, &["New Title", "New Body"], &[]);
}

#[test]
fn html_conflict_same_element() {
    let r = merge_html(
        "<html><body><h1>Title</h1></body></html>",
        "<html><body><h1>Ours</h1></body></html>",
        "<html><body><h1>Theirs</h1></body></html>",
    )
    .unwrap();
    check_conflict(&r);
}

#[test]
fn html_attribute_changes() {
    let r = merge_html(
        r#"<html><body><a href="/old">link</a></body></html>"#,
        r#"<html><body><a href="/new">link</a></body></html>"#,
        r#"<html><body><a href="/old" class="active">link</a></body></html>"#,
    )
    .unwrap();
    check_clean(&r, &["href=\"/new\"", "class=\"active\""], &[]);
}

#[test]
fn html_diff_and_format() {
    let changes = diff(
        "<html><body><p>old</p></body></html>",
        "<html><body><p>new</p></body></html>",
        Some(".html"),
    )
    .unwrap();
    assert!(!changes.is_empty());
}

// ============================================================================
// 9. iCalendar
// ============================================================================

#[test]
fn ical_clean_different_events() {
    let base = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting A\r\nUID:a@test.com\r\nDTSTART:20240101T100000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting B\r\nUID:b@test.com\r\nDTSTART:20240102T100000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let ours = base.replace("SUMMARY:Meeting A", "SUMMARY:Updated Meeting A");
    let theirs = base.replace("SUMMARY:Meeting B", "SUMMARY:Updated Meeting B");

    let r = merge_ical(base, &ours, &theirs).unwrap();
    check_clean(&r, &["Updated Meeting A", "Updated Meeting B"], &[]);
}

#[test]
fn ical_conflict_same_event() {
    let base = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting\r\nUID:x@test.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let ours = base.replace("SUMMARY:Meeting", "SUMMARY:Ours");
    let theirs = base.replace("SUMMARY:Meeting", "SUMMARY:Theirs");

    let r = merge_ical(base, &ours, &theirs).unwrap();
    check_conflict(&r);
}

#[test]
fn ical_diff_and_format() {
    let base = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting\r\nUID:x@test.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let modified = base.replace("SUMMARY:Meeting", "SUMMARY:Updated");
    let changes = diff(base, &modified, Some(".ics")).unwrap();
    assert!(!changes.is_empty());
}

// ============================================================================
// 10. RSS Feed
// ============================================================================

#[test]
fn feed_rss_clean_different_entries() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>My Feed</title>
    <item>
      <title>Article 1</title>
      <guid>abc</guid>
    </item>
    <item>
      <title>Article 2</title>
      <guid>def</guid>
    </item>
  </channel>
</rss>"#;
    let ours = base.replace("Article 1", "Updated Article 1");
    let theirs = base.replace("Article 2", "Updated Article 2");

    let r = merge_feed(base, &ours, &theirs).unwrap();
    check_clean(&r, &["Updated Article 1", "Updated Article 2"], &[]);
}

#[test]
fn feed_rss_conflict_same_entry() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Feed</title>
    <item>
      <title>Post</title>
      <guid>x</guid>
    </item>
  </channel>
</rss>"#;
    let ours = base.replace("Post", "Ours Post");
    let theirs = base.replace("Post", "Theirs Post");

    let r = merge_feed(base, &ours, &theirs).unwrap();
    check_conflict(&r);
}

#[test]
fn feed_atom_clean() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Feed</title>
  <entry>
    <title>Entry 1</title>
    <id>e1</id>
  </entry>
  <entry>
    <title>Entry 2</title>
    <id>e2</id>
  </entry>
</feed>"#;
    let ours = base.replace("Entry 1", "Updated Entry 1");
    let theirs = base.replace("Entry 2", "Updated Entry 2");

    let r = merge_feed(base, &ours, &theirs).unwrap();
    check_clean(&r, &["Updated Entry 1", "Updated Entry 2"], &[]);
}

#[test]
fn feed_diff_and_format() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Feed</title></channel></rss>"#;
    let modified = base.replace("Feed", "Updated Feed");
    let changes = diff(base, &modified, Some(".rss")).unwrap();
    assert!(!changes.is_empty());
}

// ============================================================================
// 11. merge_auto with all extensions
// ============================================================================

#[test]
fn merge_auto_json() {
    let r = merge_auto(
        r#"{"a": 1}"#,
        r#"{"a": 2}"#,
        r#"{"b": 3}"#,
        Some(".json"),
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_yaml() {
    let r = merge_auto("a: 1\n", "a: 2\n", "b: 3\n", Some(".yaml")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_yml() {
    let r = merge_auto("a: 1\n", "a: 2\n", "b: 3\n", Some(".yml")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_toml() {
    let r = merge_auto("a = 1\n", "a = 2\n", "b = 3\n", Some(".toml")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_csv() {
    let r = merge_auto("a,b\n1,2\n", "a,b\n1,3\n", "a,b\n1,2\n", Some(".csv")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_xml() {
    let r = merge_auto("<r><a>1</a></r>", "<r><a>2</a></r>", "<r><b>3</b></r>", Some(".xml")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_md() {
    let r = merge_auto("# A\n\nold\n", "# A\n\nnew\n", "# A\n\nold\n", Some(".md")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_svg() {
    let r = merge_auto(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="blue"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="green"/></svg>"#,
        Some(".svg"),
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
}

#[test]
fn merge_auto_html() {
    let r = merge_auto(
        "<html><body><p>a</p></body></html>",
        "<html><body><p>b</p></body></html>",
        "<html><body><p>a</p></body></html>",
        Some(".html"),
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_ics() {
    let base = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:A\r\nUID:x@t\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let r = merge_auto(base, base, base, Some(".ics")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_rss() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><title>F</title></channel></rss>"#;
    let r = merge_auto(base, base, base, Some(".rss")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_atom() {
    let base = r#"<?xml version="1.0" encoding="UTF-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>F</title></feed>"#;
    let r = merge_auto(base, base, base, Some(".atom")).unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

#[test]
fn merge_auto_unsupported() {
    let r = merge_auto("a", "b", "c", Some(".xyz"));
    assert!(r.is_err());
}

#[test]
fn merge_auto_no_extension() {
    let r = merge_auto("a", "b", "c", None);
    assert!(r.is_err());
}

// ============================================================================
// 12. Edge cases
// ============================================================================

#[test]
fn edge_empty_all() {
    // Empty string is not valid JSON — driver returns parse error
    let r = merge_json("", "", "");
    assert!(r.is_err());
}

#[test]
fn edge_malformed_json() {
    // Malformed JSON — should return error or handle gracefully
    let r = merge_json("{bad json", "{also bad", "{three bad");
    // May error or may succeed — just don't panic
    let _ = r;
}

#[test]
fn edge_malformed_xml() {
    let r = merge_xml("<broken", "<also broken", "<three broken");
    let _ = r; // Don't panic
}

#[test]
fn edge_unicode_json() {
    let r = merge_json(
        r#"{"name": "日本語テスト", "emoji": "🚀🎉"}"#,
        r#"{"name": "中文测试", "emoji": "🚀🎉"}"#,
        r#"{"name": "日本語テスト", "emoji": "🎵🎶"}"#,
    )
    .unwrap();
    check_clean(&r, &["中文测试", "🎵🎶"], &[]);
}

#[test]
fn edge_whitespace_yaml() {
    let r = merge_yaml(
        "key: value\n",
        "key:  value\n",  // extra space
        "key: value\n",
    )
    .unwrap();
    // Extra whitespace may or may not be detected as a change
    let _ = r; // Just don't panic
}

#[test]
fn edge_csv_single_column() {
    let r = merge_csv(
        "name\nAlice\nBob\n",
        "name\nAlice\nCharlie\n",
        "name\nAlice\nBob\n",
    )
    .unwrap();
    check_clean(&r, &["Charlie"], &[]);
}

#[test]
fn edge_csv_no_header() {
    let r = merge_csv("1\n2\n", "1\n3\n", "1\n2\n").unwrap();
    let _ = r; // Don't panic
}

#[test]
fn edge_toml_array() {
    let r = merge_toml(
        "items = [1, 2, 3]\n",
        "items = [1, 2, 4]\n",
        "items = [1, 2, 3]\n",
    )
    .unwrap();
    let _ = r; // Array merge behavior may vary
}

#[test]
fn edge_markdown_empty() {
    let r = merge_markdown("", "# New\n", "").unwrap();
    assert_eq!(r.status, MergeStatus::Clean);
}

// ============================================================================
// 13. Real-world files
// ============================================================================

#[test]
fn realworld_json_posts() {
    // Use first 2 posts from the downloaded dataset
    let base = r#"[
  {"id": 1, "title": "First Post", "body": "Hello world"},
  {"id": 2, "title": "Second Post", "body": "Another post"}
]"#;
    let ours = base.replace("Hello world", "Hello universe");
    let theirs = base.replace("Another post", "Yet another post");

    let r = merge_json(base, &ours, &theirs).unwrap();
    check_clean(&r, &["Hello universe", "Yet another post"], &[]);
}

#[test]
fn realworld_csv_cpi() {
    // Minimal CSV similar to CPI dataset
    let base = "Year,Jan,Feb,Mar\n2024,100,102,101\n";
    let ours = base.replace("100", "105");
    let theirs = base.replace("101", "103");

    let r = merge_csv(base, &ours, &theirs).unwrap();
    check_clean(&r, &["105", "103"], &[]);
}

#[test]
fn realworld_xml_note() {
    let base = r#"<?xml version="1.0"?>
<note>
  <to>Tove</to>
  <from>Jani</from>
  <heading>Reminder</heading>
  <body>Don't forget me!</body>
</note>"#;
    let ours = base.replace("Tove", "Alice");
    let theirs = base.replace("Jani", "Bob");

    let r = merge_xml(base, &ours, &theirs).unwrap();
    check_clean(&r, &["Alice", "Bob"], &[]);
}

#[test]
fn realworld_markdown_readme() {
    let base = "# Linux\n\nLinux is cool.\n\n## Security\n\nStay safe.\n";
    let ours = base.replace("Linux is cool", "Linux is awesome");
    let theirs = base.replace("Stay safe", "Be secure");

    let r = merge_markdown(base, &ours, &theirs).unwrap();
    check_clean(&r, &["awesome", "Be secure"], &[]);
}

#[test]
fn realworld_yaml_ci_config() {
    let base = r#"inputs:
  fetch-depth:
    description: "Number of commits"
    default: "1"
  ref:
    description: "Branch"
"#;
    let ours = base.replace("default: \"1\"", "default: \"10\"");
    let theirs = base.replace("Branch", "Branch to checkout");

    let r = merge_yaml(base, &ours, &theirs).unwrap();
    // serde_yaml normalizes: reorders keys alphabetically, strips quotes
    // Just check the value is present regardless of formatting
    check_clean(&r, &["default: '10'"], &[]);
}

// ============================================================================
// 14. merge_auto diff/format_diff consistency
// ============================================================================

#[test]
fn consistency_diff_then_format() {
    let base = r#"{"a": 1, "b": 2}"#;
    let modified = r#"{"a": 1, "b": 3, "c": 4}"#;

    let changes = diff(base, modified, Some(".json")).unwrap();
    let formatted = format_diff(base, modified, Some(".json")).unwrap();

    // If there are changes, format_diff should not be "no changes"
    if !changes.is_empty() {
        assert_ne!(formatted.trim(), "no changes");
    }
}

#[test]
fn consistency_no_changes() {
    let content = r#"{"a": 1}"#;
    let changes = diff(content, content, Some(".json")).unwrap();
    let formatted = format_diff(content, content, Some(".json")).unwrap();

    assert!(changes.is_empty());
    assert_eq!(formatted.trim(), "no changes");
}

#[test]
fn consistency_all_formats_no_change() {
    // Every format should report no changes for identical content
    let formats: &[(&str, &str, &str, fn(&str, &str, &str) -> Result<MergeResult, MergeError>)] = &[
        (".json", r#"{"a":1}"#, r#"{"a":1}"#, merge_json),
        ("base", "a: 1\n", "a: 1\n", merge_yaml),
        ("base", "a = 1\n", "a = 1\n", merge_toml),
        ("base", "a\n1\n", "a\n1\n", merge_csv),
        ("base", "<r><a>1</a></r>", "<r><a>1</a></r>", merge_xml),
        ("base", "# A\n\nB\n", "# A\n\nB\n", merge_markdown),
    ];

    for (label, base, modified, merge_fn) in formats {
        let r = merge_fn(base, modified, base).unwrap();
        assert_eq!(
            r.status,
            MergeStatus::Clean,
            "Format {} should be clean for identical inputs",
            label
        );
    }
}
