//! Fuzz target for WASM plugin validation and host-function ABI robustness.
//!
//! Tests that:
//! - `validate_plugin()` handles arbitrary/malformed bytes without panic
//! - The host-function ABI handles truncated/corrupted plugin output gracefully
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Test 1: validate_plugin should never panic on arbitrary bytes
    let _ = suture_wasm_plugin::validate_plugin(data);

    // Test 2: WasmPluginHost::new should not panic on malformed WASM
    let _ = suture_wasm_plugin::WasmPluginHost::new(data);

    // Test 3: With at least 8 bytes, try a minimal valid-looking module header
    // to exercise more code paths in compilation/validation
    if data.len() >= 8 {
        // Wasm magic number + version
        let mut wasm_like = data.to_vec();
        wasm_like[0] = 0x00;
        wasm_like[1] = 0x61;
        wasm_like[2] = 0x73;
        wasm_like[3] = 0x6D; // \0asm
        wasm_like[4] = 0x01;
        wasm_like[5] = 0x00;
        wasm_like[6] = 0x00;
        wasm_like[7] = 0x00; // version 1

        let _ = suture_wasm_plugin::validate_plugin(&wasm_like);
        let _ = suture_wasm_plugin::WasmPluginHost::new(&wasm_like);
    }

    // Test 4: Very short inputs
    if data.len() >= 2 {
        let _ = suture_wasm_plugin::validate_plugin(&data[..2]);
        let _ = suture_wasm_plugin::WasmPluginHost::new(&data[..2]);
    }

    // Test 5: Empty-ish module with just header
    let empty_module: &[u8] = &[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
    let _ = suture_wasm_plugin::validate_plugin(empty_module);
    let _ = suture_wasm_plugin::WasmPluginHost::new(empty_module);
});
