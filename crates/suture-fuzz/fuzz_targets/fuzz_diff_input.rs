//! Fuzz target for unified diff parser
//!
//! suture-cli is a binary crate, so parse_unified_diff is not accessible.
//! This target re-implements the same parsing logic to verify it never
//! panics on malformed diff input.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_unified_diff(s);
    }
});

fn parse_unified_diff(text: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let mut file_count = 0usize;
    let mut in_file = false;
    let mut _hunk_header: Option<(usize, usize, usize, usize)> = None;

    for line in text.lines() {
        if let Some(_rest) = line.strip_prefix("--- ") {
            if in_file {
                file_count += 1;
            }
            in_file = true;
            continue;
        }
        if line.starts_with("+++ ") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("@@ ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            let parse_range = |s: &str| -> Option<(usize, usize)> {
                let s = s.trim_start_matches('-').trim_start_matches('+');
                let mut iter = s.splitn(2, ',');
                let start: usize = iter.next()?.parse().ok()?;
                let count: usize = iter.next().and_then(|n| n.parse().ok()).unwrap_or(1);
                Some((start, count))
            };
            let old_range = parts.first().and_then(|s| parse_range(s));
            let new_range = parts.get(1).and_then(|s| parse_range(s));
            if let (Some((os, oc)), Some((ns, nc))) = (old_range, new_range) {
                _hunk_header = Some((os, oc, ns, nc));
            }
            continue;
        }
        if _hunk_header.is_some() {
            let _ch = line.chars().next();
        }
    }

    if in_file {
        file_count += 1;
    }

    Ok(file_count)
}
