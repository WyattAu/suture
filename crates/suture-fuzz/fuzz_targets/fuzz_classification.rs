//! Fuzz target for classification pattern detection
//!
//! suture-cli is a binary crate, so detect_classification is not accessible.
//! This target directly tests the regex patterns used for classification
//! marking detection to ensure they never panic on arbitrary input.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let lines: Vec<&str> = s.lines().take(50).collect();
        let patterns: &[&str] = &[
            r"(?i)TOP\s+SECRET\s*//\s*SCI",
            r"(?i)\bTOP\s+SECRET\b",
            r"(?i)\bSECRET\b",
            r"(?i)\bCONFIDENTIAL\b",
            r"(?i)\bOFFICIAL[\s-]SENSITIVE\b",
            r"(?i)\bOFFICIAL\b",
            r"(?i)\bRESTRICTED\b",
            r"(?i)\bPROTECTED\b",
            r"(?i)\bCUI\b",
            r"(?i)\bUNCLASSIFIED\b",
            r"(?i)\bFOR\s+OFFICIAL\s+USE\s+ONLY\b",
            r"(?i)\bCOMMERCIAL\s+IN\s+CONFIDENCE\b",
            r"(?i)\bPRIVILEGED\s+AND\s+CONFIDENTIAL\b",
        ];
        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                for line in &lines {
                    let _ = re.find_iter(line).count();
                }
            }
        }
    }
});
