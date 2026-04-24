//! Fuzz target for patch deserialization
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<suture_core::patch::types::Patch>(s);
    }
});
