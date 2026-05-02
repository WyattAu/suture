//! Fuzz target for TOML semantic merge
#![no_main]
use libfuzzer_sys::fuzz_target;
use suture_driver::SutureDriver;

fuzz_target!(|data: &[u8]| {
    let parts: Vec<&[u8]> = data.splitn(3, |&b| b == 0).collect();
    if parts.len() < 3 {
        return;
    }

    let Ok(base) = std::str::from_utf8(parts[0]) else { return };
    let Ok(ours) = std::str::from_utf8(parts[1]) else { return };
    let Ok(theirs) = std::str::from_utf8(parts[2]) else { return };

    let driver = suture_driver_toml::TomlDriver::new();
    let _ = driver.merge(base, ours, theirs);
});
