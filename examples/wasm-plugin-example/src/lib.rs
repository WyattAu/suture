//! Example Suture WASM plugin that merges JSON with custom conflict resolution.
//!
//! This demonstrates the host-function ABI v1. The host makes input available
//! through `get_input_len` / `get_input_byte` imports (from the `"env"` module)
//! and the plugin writes its output via `set_output_len` / `set_output_byte`.
//!
//! # Build
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown
//! ```
//!
//! The resulting `.wasm` file can be loaded by `suture-wasm-plugin::WasmPluginHost`.

extern "C" {
    /// Returns the length of the JSON input buffer.
    fn get_input_len() -> i32;
    /// Returns one byte from the JSON input buffer at the given offset.
    /// Returns -1 if the offset is out of bounds.
    fn get_input_byte(offset: i32) -> i32;
    /// Writes one byte to the output buffer at the given offset.
    fn set_output_byte(offset: i32, byte: i32);
    /// Sets the length of the output buffer.
    fn set_output_len(len: i32);
    /// Log a message to the host. Level: 0=trace, 1=debug, 2=info, 3=warn, 4+=error.
    fn host_log(level: i32, msg_ptr: *const u8, msg_len: i32);
}

/// Read the full JSON input from the host.
fn read_input() -> String {
    // SAFETY: get_input_len() is a host-provided FFI import with no preconditions.
    let len = unsafe { get_input_len() } as usize;
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len {
        // SAFETY: offset i is in 0..len where len was returned by get_input_len(),
        // satisfying the host ABI contract for get_input_byte.
        bytes.push(unsafe { get_input_byte(i as i32) } as u8);
    }
    String::from_utf8(bytes).unwrap_or_default()
}

/// Write the merge result to the output buffer.
fn write_output(s: &str) {
    let bytes = s.as_bytes();
    // SAFETY: set_output_len() declares the output buffer size to the host.
    // The subsequent loop writes within this range.
    unsafe { set_output_len(bytes.len() as i32); }
    for (i, &byte) in bytes.iter().enumerate() {
        // SAFETY: offset i is within the range declared by set_output_len() above,
        // satisfying the host ABI contract for set_output_byte.
        unsafe { set_output_byte(i as i32, byte as i32); }
    }
}

/// Set the error message before returning -1.
fn set_error(msg: &str) {
    let bytes = msg.as_bytes();
    // SAFETY: bytes.as_ptr() points to valid UTF-8 (from &str), and bytes.len()
    // is the correct byte length. The pointer remains valid for the duration
    // of this call since no mutation occurs.
    unsafe {
        host_log(4, bytes.as_ptr(), bytes.len() as i32);
    }
}

/// Merge function called by the host.
///
/// The host provides JSON `{"base":…,"ours":…,"theirs":…}` via the
/// `get_input_*` imports. This plugin applies a simple strategy:
/// if `ours` differs from `base`, keep `ours`; otherwise take `theirs`.
///
/// Returns: `0` = merged, `1` = conflict, `-1` = error.
#[no_mangle]
pub extern "C" fn suture_merge() -> i32 {
    let input = read_input();

    let data: serde_json::Value = match serde_json::from_str(&input) {
        Ok(d) => d,
        Err(e) => {
            set_error(&format!("invalid JSON: {e}"));
            return -1;
        }
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

/// ABI version — must match PLUGIN_ABI_VERSION (1).
#[no_mangle]
pub extern "C" fn suture_abi_version() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn suture_plugin_name() -> *const u8 {
    b"suture-example".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_plugin_name_len() -> i32 {
    14
}

#[no_mangle]
pub extern "C" fn suture_plugin_version() -> *const u8 {
    b"0.1.0".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_plugin_version_len() -> i32 {
    5
}

/// Comma-separated list of file extensions this plugin handles.
#[no_mangle]
pub extern "C" fn suture_extensions() -> *const u8 {
    b"json".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_extensions_len() -> i32 {
    4
}

/// Error message (set before returning -1 from suture_merge).
#[no_mangle]
pub extern "C" fn suture_error_msg() -> *const u8 {
    // This is a placeholder — a real plugin would store the error in a static buffer
    b"unknown error".as_ptr()
}

#[no_mangle]
pub extern "C" fn suture_error_msg_len() -> i32 {
    13
}
