//! Example Suture WASM plugin that merges JSON with custom conflict resolution.
//!
//! This demonstrates the host-function ABI. The host makes input available
//! through `env_get_input_len` / `env_get_input_byte` imports and the plugin
//! writes its output via `env_set_output_len` / `env_set_output_byte`.
//!
//! # Build
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown
//! ```
//!
//! The resulting `.wasm` file can be loaded by `suture-wasm-plugin::WasmPluginHost`.

extern "C" {
    fn env_get_input_len() -> i32;
    fn env_get_input_byte(offset: i32) -> i32;
    fn env_set_output_byte(offset: i32, byte: i32);
    fn env_set_output_len(len: i32);
}

fn read_input() -> String {
    let len = unsafe { env_get_input_len() } as usize;
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len {
        bytes.push(unsafe { env_get_input_byte(i as i32) } as u8);
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn write_output(s: &str) {
    let bytes = s.as_bytes();
    unsafe { env_set_output_len(bytes.len() as i32); }
    for (i, &byte) in bytes.iter().enumerate() {
        unsafe { env_set_output_byte(i as i32, byte as i32); }
    }
}

/// Merge function called by the host.
///
/// The host provides JSON `{"base":…,"ours":…,"theirs":…}` via the
/// `env_get_input_*` imports. This plugin applies a simple strategy:
/// if `ours` differs from `base`, keep `ours`; otherwise take `theirs`.
///
/// Returns: `0` = merged, `1` = conflict, `-1` = error.
#[no_mangle]
pub extern "C" fn suture_merge() -> i32 {
    let input = read_input();

    let data: serde_json::Value = match serde_json::from_str(&input) {
        Ok(d) => d,
        Err(_) => return -1,
    };

    let base = data.get("base").and_then(|v| v.as_str()).unwrap_or("");
    let ours = data.get("ours").and_then(|v| v.as_str()).unwrap_or("");
    let theirs = data.get("theirs").and_then(|v| v.as_str()).unwrap_or("");

    if ours == base && theirs != base {
        write_output(theirs);
    } else {
        write_output(ours);
    }
    0
}

#[no_mangle]
pub extern "C" fn suture_plugin_name() -> *const u8 {
    b"suture-example\0".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_plugin_name_len() -> i32 {
    14
}

#[no_mangle]
pub extern "C" fn suture_plugin_version() -> *const u8 {
    b"0.1.0\0".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_plugin_version_len() -> i32 {
    5
}
