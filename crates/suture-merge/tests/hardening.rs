//! Hardening & stress test suite for suture-merge.
//!
//! Phases:
//! 1. Adversarial input (malformed, truncated, binary, whitespace)
//! 2. Unicode stress (BOMs, combining, RTL, surrogate, ZW chars)
//! 3. Size/stress (large files, deep nesting, long lines, many keys)
//! 4. Trivial merge cases (identity, empty, no-change)
//! 5. Cross-driver consistency
//! 6. Error quality (no panics, useful messages)
//! 7. Conflict result quality

use suture_merge::*;

// ============================================================================
// Helpers
// ============================================================================

/// Must NOT panic. Returns Ok(merge_result) or Err(error). Both are fine.
fn no_panic_merge(
    merge_fn: fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<MergeResult, MergeError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        merge_fn(base, ours, theirs)
    }));
    match result {
        Ok(r) => r,
        Err(_) => panic!(
            "PANIC on input base={} ours={} theirs={}",
            base.len(),
            ours.len(),
            theirs.len()
        ),
    }
}

fn no_panic_diff(
    ext: &str,
    base: &str,
    modified: &str,
) -> Result<Vec<suture_driver::SemanticChange>, MergeError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        diff(base, modified, Some(ext))
    }));
    match result {
        Ok(r) => r,
        Err(_) => panic!(
            "PANIC in diff for ext={} base={} modified={}",
            ext,
            base.len(),
            modified.len()
        ),
    }
}

fn no_panic_format_diff(ext: &str, base: &str, modified: &str) -> Result<String, MergeError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        format_diff(base, modified, Some(ext))
    }));
    match result {
        Ok(r) => r,
        Err(_) => panic!("PANIC in format_diff for ext={}", ext),
    }
}

// ============================================================================
// Phase 1: Adversarial Input — must not panic on ANY of these
// ============================================================================

macro_rules! adversarial_test {
    ($name:ident, $merge_fn:expr, $ext:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn empty_string() {
                let _ = no_panic_merge($merge_fn, "", "", "");
            }

            #[test]
            fn whitespace_only() {
                let _ = no_panic_merge($merge_fn, "   ", "   ", "   ");
            }

            #[test]
            fn newlines_only() {
                let _ = no_panic_merge($merge_fn, "\n\n\n", "\n\n\n", "\n\n\n");
            }

            #[test]
            fn null_bytes() {
                let input = "hello\0world";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn single_char() {
                let _ = no_panic_merge($merge_fn, "x", "y", "z");
            }

            #[test]
            fn truncated_json_like() {
                let _ = no_panic_merge($merge_fn, "{\"key\":", "{\"key\":", "{\"key\":");
            }

            #[test]
            fn random_binary() {
                let bytes: Vec<u8> = (0u8..255).collect();
                let input = String::from_utf8_lossy(&bytes);
                let _ = no_panic_merge($merge_fn, &input, &input, &input);
            }

            #[test]
            fn very_long_line() {
                let long_line = "x".repeat(100_000);
                let _ = no_panic_merge($merge_fn, &long_line, &long_line, &long_line);
            }

            #[test]
            fn mixed_newlines_crlf() {
                let input = "a\r\nb\r\nc\r\n";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn only_close_bracket() {
                let _ = no_panic_merge($merge_fn, "}", "}", "}");
            }

            #[test]
            fn deeply_nested_brackets() {
                let input = "[[[[[[[[[[{}]]]]]]]]]";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn special_chars_in_strings() {
                let input = "{\"key\": \"\\t\\n\\r\\\"\\\\\\x00\"}";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn base_valid_ours_garbage() {
                let base = r#"{"a": 1}"#;
                let garbage = "<>{}[]|\\^~`@#$%^&*()";
                let _ = no_panic_merge($merge_fn, base, garbage, base);
            }

            #[test]
            fn base_valid_theirs_garbage() {
                let base = r#"{"a": 1}"#;
                let garbage = "<>{}[]|\\^~`@#$%^&*()";
                let _ = no_panic_merge($merge_fn, base, base, garbage);
            }

            #[test]
            fn all_different() {
                let _ = no_panic_merge($merge_fn, "a", "b", "c");
            }

            #[test]
            fn unicode_bom_utf8() {
                let bom = "\u{feff}";
                let input = format!("{}{}", bom, r#"{"a": 1}"#);
                let _ = no_panic_merge($merge_fn, &input, &input, &input);
            }

            #[test]
            fn tab_indented() {
                let input = "\t\tkey: value\n\t\tkey2: value2\n";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn comment_only() {
                let input = "# just a comment\n";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn html_script_tags() {
                let input = "<html><script>alert('xss')</script></html>";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn xml_processing_instruction() {
                let input = "<?xml version=\"1.0\"?>\n<root/>";
                let _ = no_panic_merge($merge_fn, input, input, input);
            }

            #[test]
            fn diff_on_malformed() {
                let _ = no_panic_diff($ext, "garbage", "also garbage");
            }

            #[test]
            fn format_diff_on_malformed() {
                let _ = no_panic_format_diff($ext, "garbage", "also garbage");
            }
        }
    };
}

adversarial_test!(json, merge_json, ".json");
adversarial_test!(yaml, merge_yaml, ".yaml");
adversarial_test!(toml, merge_toml, ".toml");
adversarial_test!(csv, merge_csv, ".csv");
#[cfg(feature = "xml")]
adversarial_test!(xml, merge_xml, ".xml");
#[cfg(feature = "markdown")]
adversarial_test!(markdown, merge_markdown, ".md");
#[cfg(feature = "svg")]
adversarial_test!(svg, merge_svg, ".svg");
#[cfg(feature = "html")]
adversarial_test!(html, merge_html, ".html");
#[cfg(feature = "ical")]
adversarial_test!(ical, merge_ical, ".ics");
#[cfg(feature = "feed")]
adversarial_test!(feed, merge_feed, ".rss");

// ============================================================================
// Phase 2: Unicode Stress Testing
// ============================================================================

#[test]
fn unicode_chinese() {
    let r = merge_json(
        r#"{"名字": "张三", "年龄": 30}"#,
        r#"{"名字": "张三", "年龄": 31}"#,
        r#"{"名字": "李四", "年龄": 30}"#,
    );
    // Should not panic
    let _ = r;
}

#[test]
fn unicode_arabic() {
    let r = merge_yaml(
        "اسم: أحمد\nالعمر: 25\n",
        "اسم: أحمد\nالعمر: 26\n",
        "اسم: محمد\nالعمر: 25\n",
    );
    let _ = r;
}

#[test]
fn unicode_emoji() {
    let r = merge_json(
        r#"{"emoji": "🚀🎉💣", "mood": "😊"}"#,
        r#"{"emoji": "🚀🎉💣", "mood": "😴"}"#,
        r#"{"emoji": "🎮🎯🎲", "mood": "😊"}"#,
    );
    let _ = r;
}

#[test]
fn unicode_mixed_scripts() {
    let r = merge_json(
        r#"{"en": "hello", "ja": "こんにちは", "ko": "안녕하세요", "ru": "привет"}"#,
        r#"{"en": "hi", "ja": "こんにちは", "ko": "안녕하세요", "ru": "привет"}"#,
        r#"{"en": "hello", "ja": "こんにちは", "ko": "안녕하세요", "ru": "здравствуйте"}"#,
    );
    let _ = r;
}

#[test]
fn unicode_zero_width_chars() {
    let input = format!(
        "{{\"key\": \"val{}{}{}\"}}",
        '\u{200b}', '\u{200c}', '\u{200d}'
    );
    let r = merge_json(&input, &input, &input);
    let _ = r;
}

#[test]
fn unicode_right_to_left() {
    let r = merge_json(
        r#"{"مرحبا": "بسم الله"}"#,
        r#"{"مرحبا": "بسم الله الرحمن"}"#,
        r#"{"مرحبا": "بسم الله"}"#,
    );
    let _ = r;
}

#[test]
fn unicode_combining_characters() {
    // é can be 1 char (U+00E9) or 2 chars (e + U+0301)
    let r = merge_json(
        r#"{"café": "déjà vu"}"#,
        r#"{"café": "déjà vu"}"#,
        r#"{"café": "déjà vu"}"#,
    );
    let _ = r;
}

#[test]
fn unicode_surrogate_pairs() {
    // Emoji with skin tone modifier
    let r = merge_json(
        r#"{"wave": "👋🏻"}"#,
        r#"{"wave": "👋🏻"}"#,
        r#"{"wave": "👋🏻"}"#,
    );
    let _ = r;
}

#[test]
fn unicode_max_codepoint() {
    let r = merge_json(
        r#"{"sym": "\u{10FFFF}"}"#,
        r#"{"sym": "\u{10FFFF}"}"#,
        r#"{"sym": "\u{10FFFF}"}"#,
    );
    let _ = r;
}

// ============================================================================
// Phase 3: Size / Stress Testing
// ============================================================================

#[test]
fn stress_json_1000_fields() {
    let mut parts: Vec<String> = vec!["{".to_string()];
    for i in 0..1000 {
        if i > 0 {
            parts.push(",".to_string());
        }
        parts.push(format!(r#""f{i}": {i}"#));
    }
    parts.push("}".to_string());
    let content = parts.join("");

    let r = merge_json(&content, &content, &content);
    assert!(
        r.is_ok(),
        "1000-field JSON merge should succeed: {:?}",
        r.err()
    );
}

#[test]
fn stress_json_deep_nesting_100() {
    let mut base = String::from("{\"v\":");
    for _ in 0..100 {
        base.push_str("{\"v\":");
    }
    base.push_str("0");
    for _ in 0..100 {
        base.push_str("}");
    }
    base.push_str("}");

    let r = merge_json(&base, &base, &base);
    assert!(
        r.is_ok(),
        "100-deep nested JSON merge should succeed: {:?}",
        r.err()
    );
}

#[test]
fn stress_csv_10000_rows() {
    let mut base = String::from("id,name,value\n");
    for i in 0..10000 {
        base.push_str(&format!("{},row{},{}\n", i, i, i * 10));
    }
    let ours = base.replace("row0", "ROW_ZERO");
    let theirs = base.replace("row9999", "ROW_LAST");

    let r = merge_csv(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "10000-row CSV merge should succeed: {:?}",
        r.err()
    );
    if let Ok(result) = r {
        assert_eq!(result.status, MergeStatus::Clean);
    }
}

#[cfg(feature = "xml")]
#[test]
fn stress_xml_500_elements() {
    let mut base = String::from("<root>");
    for i in 0..500 {
        base.push_str(&format!("<item id=\"{}\">val{}</item>", i, i));
    }
    base.push_str("</root>");

    let ours = base.replace("val0", "MODIFIED_0");
    let theirs = base.replace("val499", "MODIFIED_499");

    let r = merge_xml(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "500-element XML merge should succeed: {:?}",
        r.err()
    );
    if let Ok(result) = r {
        assert_eq!(result.status, MergeStatus::Clean);
    }
}

#[cfg(feature = "markdown")]
#[test]
fn stress_markdown_200_sections() {
    let mut base = String::new();
    for i in 0..200 {
        base.push_str(&format!(
            "# Section {}\n\nContent for section {}.\n\n",
            i, i
        ));
    }
    let ours = base.replace("Content for section 0", "MODIFIED section 0");
    let theirs = base.replace("Content for section 199", "MODIFIED section 199");

    let r = merge_markdown(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "200-section Markdown merge should succeed: {:?}",
        r.err()
    );
}

#[test]
fn stress_yaml_500_keys() {
    let mut base = String::new();
    for i in 0..500 {
        base.push_str(&format!("key{}: value{}\n", i, i));
    }
    let ours = base.replace("value0", "MODIFIED_0");
    let theirs = base.replace("value499", "MODIFIED_499");

    let r = merge_yaml(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "500-key YAML merge should succeed: {:?}",
        r.err()
    );
}

#[cfg(feature = "ical")]
#[test]
fn stress_ical_50_events() {
    let mut base = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n");
    for i in 0..50 {
        base.push_str(&format!(
            "BEGIN:VEVENT\r\nSUMMARY:Event {}\r\nUID:event{}@test.com\r\nEND:VEVENT\r\n",
            i, i
        ));
    }
    base.push_str("END:VCALENDAR\r\n");

    let ours = base.replace("Event 0", "MODIFIED Event 0");
    let theirs = base.replace("Event 49", "MODIFIED Event 49");

    let r = merge_ical(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "50-event iCal merge should succeed: {:?}",
        r.err()
    );
}

#[cfg(feature = "feed")]
#[test]
fn stress_feed_50_entries() {
    let mut items = String::new();
    for i in 0..50 {
        items.push_str(&format!(
            "    <item><title>Article {}</title><guid>id{}</guid></item>\n",
            i, i
        ));
    }
    let base = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\">\n  <channel>\n    <title>Feed</title>\n{}\n  </channel>\n</rss>",
        items
    );
    let ours = base.replace("Article 0", "MODIFIED Article 0");
    let theirs = base.replace("Article 49", "MODIFIED Article 49");

    let r = merge_feed(&base, &ours, &theirs);
    assert!(
        r.is_ok(),
        "50-entry feed merge should succeed: {:?}",
        r.err()
    );
}

// ============================================================================
// Phase 4: Trivial Merge Cases
// ============================================================================

macro_rules! trivial_test {
    ($name:ident, $merge_fn:expr, $valid:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn base_equals_ours_equals_theirs() {
                let r = no_panic_merge($merge_fn, $valid, $valid, $valid).unwrap();
                assert_eq!(r.status, MergeStatus::Clean);
            }

            #[test]
            fn base_equals_ours() {
                // theirs changed, ours didn't → theirs wins
                let r = no_panic_merge($merge_fn, $valid, $valid, "DIFFERENT");
                // May be Clean or ParseError for "DIFFERENT", but never panic
            }

            #[test]
            fn base_equals_theirs() {
                // ours changed, theirs didn't → ours wins
                let r = no_panic_merge($merge_fn, $valid, "DIFFERENT", $valid);
            }

            #[test]
            fn ours_equals_theirs() {
                // Both made the same change → clean
                let r = no_panic_merge($merge_fn, $valid, "SAME", "SAME");
            }
        }
    };
}

trivial_test!(json_trivial, merge_json, r#"{"a": 1}"#);
trivial_test!(yaml_trivial, merge_yaml, "a: 1\n");
trivial_test!(toml_trivial, merge_toml, "a = 1\n");
trivial_test!(csv_trivial, merge_csv, "a,b\n1,2\n");
#[cfg(feature = "xml")]
trivial_test!(xml_trivial, merge_xml, "<r><a>1</a></r>");
#[cfg(feature = "markdown")]
trivial_test!(md_trivial, merge_markdown, "# A\n\nB\n");
#[cfg(feature = "svg")]
trivial_test!(
    svg_trivial,
    merge_svg,
    r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r"/></svg>"#
);
#[cfg(feature = "html")]
trivial_test!(
    html_trivial,
    merge_html,
    "<html><body><p>a</p></body></html>"
);

// ============================================================================
// Phase 5: Cross-Driver Consistency
// ============================================================================

/// All drivers should handle "no change" identically
#[test]
fn consistency_all_drivers_no_change() {
    let mut inputs: Vec<(
        &str,
        &str,
        fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    )> = vec![
        (
            ".json",
            r#"{"a": 1}"#,
            merge_json as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
        ),
        (
            ".yaml",
            "a: 1\n",
            merge_yaml as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
        ),
        (
            ".toml",
            "a = 1\n",
            merge_toml as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
        ),
        (
            ".csv",
            "a\n1\n",
            merge_csv as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
        ),
    ];
    #[cfg(feature = "xml")]
    inputs.push((
        ".xml",
        "<r><a>1</a></r>",
        merge_xml as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    ));
    #[cfg(feature = "markdown")]
    inputs.push((
        ".md",
        "# A\n\nB\n",
        merge_markdown as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    ));
    #[cfg(feature = "svg")]
    inputs.push((
        ".svg",
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r"/></svg>"#,
        merge_svg as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    ));
    #[cfg(feature = "html")]
    inputs.push((
        ".html",
        "<html><p>a</p></html>",
        merge_html as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    ));

    for (label, content, merge_fn) in inputs {
        let r = merge_fn(content, content, content).unwrap();
        assert_eq!(
            r.status,
            MergeStatus::Clean,
            "{}: identical inputs should be Clean",
            label
        );
    }
}

/// All XML-like drivers (XML, SVG, HTML) should handle the same basic structure
#[cfg(feature = "xml")]
#[test]
fn consistency_xml_family_basic() {
    let base = r#"<root><a>1</a><b>2</b></root>"#;
    let ours = r#"<root><a>10</a><b>2</b></root>"#;
    let theirs = r#"<root><a>1</a><b>20</b></root>"#;

    let mut drivers: Vec<(
        &str,
        fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    )> = vec![(
        "xml",
        merge_xml as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    )];
    #[cfg(feature = "html")]
    drivers.push((
        "html",
        merge_html as fn(&str, &str, &str) -> Result<MergeResult, MergeError>,
    ));

    for (label, merge_fn) in drivers {
        let r = merge_fn(base, ours, theirs).unwrap();
        assert_eq!(
            r.status,
            MergeStatus::Clean,
            "{}: different-element merge should be Clean",
            label
        );
    }
}

/// merge_auto should match the format-specific function for every extension
#[test]
fn consistency_auto_matches_specific() {
    let base = r#"{"a": 1, "b": 2}"#;
    let ours = r#"{"a": 10, "b": 2}"#;
    let theirs = r#"{"a": 1, "b": 20}"#;

    let specific = merge_json(base, ours, theirs).unwrap();
    let auto = merge_auto(base, ours, theirs, Some(".json")).unwrap();

    assert_eq!(
        specific.status, auto.status,
        "merge_auto should match merge_json status"
    );
}

// ============================================================================
// Phase 6: Error Quality
// ============================================================================

#[test]
fn error_merge_auto_unsupported_has_message() {
    let err = merge_auto("a", "b", "c", Some(".xyz")).unwrap_err();
    let msg = err.to_string();
    assert!(!msg.is_empty(), "Error message should not be empty");
    assert!(
        msg.len() > 5,
        "Error message should be descriptive, got: {}",
        msg
    );
}

#[test]
fn error_merge_auto_no_extension_has_message() {
    let err = merge_auto("a", "b", "c", None).unwrap_err();
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

#[test]
fn error_diff_unsupported_has_message() {
    let err = diff("a", "b", Some(".xyz")).unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn error_format_diff_unsupported_has_message() {
    let err = format_diff("a", "b", Some(".xyz")).unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn error_malformed_json_is_parse_error() {
    let r = merge_json("{bad", "{bad", "{bad");
    match r {
        Err(MergeError::ParseError(_)) => {} // expected
        Ok(_) => {}                          // also acceptable (driver may handle gracefully)
        Err(e) => panic!("Expected ParseError or Ok, got: {:?}", e),
    }
}

#[cfg(feature = "xml")]
#[test]
fn error_malformed_xml_is_parse_error() {
    let r = merge_xml("<broken", "<broken", "<broken");
    match r {
        Err(MergeError::ParseError(_)) => {}
        Ok(_) => {}
        Err(e) => panic!("Expected ParseError or Ok, got: {:?}", e),
    }
}

// ============================================================================
// Phase 7: Conflict Result Quality
// ============================================================================

#[test]
fn conflict_result_has_merged_content() {
    let r = merge_json(
        r#"{"key": "original"}"#,
        r#"{"key": "ours"}"#,
        r#"{"key": "theirs"}"#,
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(
        !r.merged.is_empty(),
        "Conflict result should have merged content (even if best-effort)"
    );
}

#[test]
fn conflict_result_json_contains_one_side() {
    let r = merge_json(
        r#"{"key": "original"}"#,
        r#"{"key": "ours"}"#,
        r#"{"key": "theirs"}"#,
    )
    .unwrap();
    // Best-effort: merged should contain at least one side's value
    let has_ours = r.merged.contains("ours");
    let has_theirs = r.merged.contains("theirs");
    assert!(
        has_ours || has_theirs,
        "Conflict merged should contain ours or theirs, got: {}",
        r.merged
    );
}

#[test]
fn conflict_result_yaml_has_content() {
    let r = merge_yaml("key: original\n", "key: ours\n", "key: theirs\n").unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[test]
fn conflict_result_toml_has_content() {
    let r = merge_toml(
        "key = \"original\"\n",
        "key = \"ours\"\n",
        "key = \"theirs\"\n",
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[test]
fn conflict_result_csv_has_content() {
    let r = merge_csv("a,b\n1,2\n", "a,b\n1,3\n", "a,b\n1,4\n").unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "xml")]
#[test]
fn conflict_result_xml_has_content() {
    let r = merge_xml(
        "<root><item>x</item></root>",
        "<root><item>ours</item></root>",
        "<root><item>theirs</item></root>",
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "markdown")]
#[test]
fn conflict_result_markdown_has_content() {
    let r = merge_markdown("# A\n\nBody\n", "# A\n\nOurs\n", "# A\n\nTheirs\n").unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "svg")]
#[test]
fn conflict_result_svg_has_content() {
    let r = merge_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="red"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="blue"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" fill="green"/></svg>"#,
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "html")]
#[test]
fn conflict_result_html_has_content() {
    let r = merge_html(
        "<html><body><p>original</p></body></html>",
        "<html><body><p>ours</p></body></html>",
        "<html><body><p>theirs</p></body></html>",
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "ical")]
#[test]
fn conflict_result_ical_has_content() {
    let base = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting\r\nUID:x@t\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let r = merge_ical(
        base,
        &base.replace("Meeting", "Ours"),
        &base.replace("Meeting", "Theirs"),
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}

#[cfg(feature = "feed")]
#[test]
fn conflict_result_feed_has_content() {
    let base = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>F</title><item><title>P</title><guid>x</guid></item></channel></rss>"#;
    let r = merge_feed(
        base,
        &base.replace("P", "Ours"),
        &base.replace("P", "Theirs"),
    )
    .unwrap();
    assert_eq!(r.status, MergeStatus::Conflict);
    assert!(!r.merged.is_empty());
}
