//! Fuzz target for TOML semantic merge
#![no_main]
use libfuzzer_sys::fuzz_target;
use suture_driver::SutureDriver;

fuzz_target!(|data: &[u8]| {
    let parts: Vec<&[u8]> = data.splitn(3, |&b| b == 0).collect();
    if parts.len() < 3 {
        return;
    }

    let base = match std::str::from_utf8(parts[0]) {
        Ok(s) => s,
        Err(_) => return,
    };
    let ours = match std::str::from_utf8(parts[1]) {
        Ok(s) => s,
        Err(_) => return,
    };
    let theirs = match std::str::from_utf8(parts[2]) {
        Ok(s) => s,
        Err(_) => return,
    };

    let driver = suture_driver_toml::TomlDriver::new();
    let _ = driver.merge(base, ours, theirs);
});
