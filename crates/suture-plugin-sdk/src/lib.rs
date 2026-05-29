//! # Suture Plugin SDK
//!
//! Safe Rust wrappers for writing WASM merge plugins that run inside the
//! Suture Hub host-function ABI v1.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use suture_plugin_sdk::prelude::*;
//!
//! #[no_mangle]
//! pub extern "C" fn suture_merge() -> i32 {
//!     let input = read_input();
//!     // ... parse JSON, merge, write output ...
//!     write_output(&merged);
//!     0
//! }
//! ```
//!
//! ## Required Exports
//!
//! Your plugin must export the following functions:
//!
//! | Export | Signature | Description |
//! |--------|-----------|-------------|
//! | `suture_merge` | `() -> i32` | Perform merge (0=ok, 1=conflict, -1=error) |
//! | `suture_abi_version` | `() -> i32` | Must return `1` |
//! | `suture_plugin_name` | `() -> *const u8` | Plugin name bytes |
//! | `suture_plugin_name_len` | `() -> i32` | Name length |
//! | `suture_plugin_version` | `() -> *const u8` | Version bytes |
//! | `suture_plugin_version_len` | `() -> i32` | Version length |
//! | `suture_extensions` | `() -> *const u8` | Comma-separated extensions |
//! | `suture_extensions_len` | `() -> i32` | Extensions string length |
//! | `suture_error_msg` | `() -> *const u8` | Error message bytes |
//! | `suture_error_msg_len` | `() -> i32` | Error message length |

/// ABI version that plugins must report.
pub const ABI_VERSION: i32 = 1;

// ---------------------------------------------------------------------------
// Host imports (linker: "env")
// ---------------------------------------------------------------------------
// These are only resolved when compiling to wasm32-unknown-unknown.
// For host-side tests, we provide no-op stubs.

#[cfg(target_arch = "wasm32")]
mod host {
    extern "C" {
        pub fn get_input_len() -> i32;
        pub fn get_input_byte(offset: i32) -> i32;
        pub fn set_output_byte(offset: i32, byte: i32);
        pub fn set_output_len(len: i32);
        pub fn host_log(level: i32, msg_ptr: *const u8, msg_len: i32);
    }
}

// # Safety
//
// These functions are called by the WASM plugin runtime via FFI.
// The caller must ensure:
// - `get_input_byte` is only called with valid offsets (< `get_input_len()`)
// - `set_output_byte` and `set_output_len` are called before the plugin returns
// - `host_log` receives a valid UTF-8 pointer and length
//
// The host-side wrappers in this module (lines 90-135) enforce these
// invariants, making them safe to call from Rust code.

#[cfg(not(target_arch = "wasm32"))]
mod host {
    // SAFETY: These are no-op stubs used only for compilation on non-WASM
    // targets. The real implementations are provided by the WASM host.
    // On native, the SDK is never actually called, so the stubs are safe.
    #[no_mangle]
    pub unsafe extern "C" fn get_input_len() -> i32 {
        0
    }
    // SAFETY: See get_input_len comment above.
    #[no_mangle]
    pub unsafe extern "C" fn get_input_byte(_offset: i32) -> i32 {
        -1
    }
    // SAFETY: See get_input_len comment above.
    #[no_mangle]
    pub unsafe extern "C" fn set_output_byte(_offset: i32, _byte: i32) {}
    // SAFETY: See get_input_len comment above.
    #[no_mangle]
    pub unsafe extern "C" fn set_output_len(_len: i32) {}
    // SAFETY: See get_input_len comment above.
    #[no_mangle]
    pub unsafe extern "C" fn host_log(_level: i32, _msg_ptr: *const u8, _msg_len: i32) {}
}

// ---------------------------------------------------------------------------
// Log levels
// ---------------------------------------------------------------------------

/// Log level for [`log`].
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

/// Log a message to the host.
///
/// # Safety
/// This is safe to call from within `suture_merge`.
pub fn log(level: LogLevel, msg: &str) {
    let bytes = msg.as_bytes();
    // SAFETY: `msg` is a &str so its bytes are guaranteed valid UTF-8.
    // The pointer from as_bytes().as_ptr() is valid for the string's lifetime,
    // and bytes.len() is the exact byte length. No mutation occurs between
    // pointer creation and this call.
    unsafe {
        host::host_log(level as i32, bytes.as_ptr(), bytes.len() as i32);
    }
}

// ---------------------------------------------------------------------------
// Input / Output
// ---------------------------------------------------------------------------

/// Read the full JSON input provided by the host.
///
/// The input is a JSON string `{"base": "...", "ours": "...", "theirs": "..."}`.
pub fn read_input() -> String {
    let len = unsafe {
        // SAFETY: get_input_len returns the byte count of the input buffer.
        // We only call get_input_byte with offsets 0..len.
        host::get_input_len()
    } as usize;
    if len == 0 {
        return String::new();
    }
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len {
        // SAFETY: offset is bounded by len from get_input_len() above.
        let byte = unsafe { host::get_input_byte(i as i32) };
        if byte < 0 {
            break;
        }
        bytes.push(byte as u8);
    }
    String::from_utf8(bytes).unwrap_or_default()
}

/// Write the merge result to the output buffer.
pub fn write_output(s: &str) {
    let bytes = s.as_bytes();
    // SAFETY: set_output_len declares the buffer size to the host.
    unsafe {
        host::set_output_len(bytes.len() as i32);
    }
    for (i, &byte) in bytes.iter().enumerate() {
        // SAFETY: offset is within the range declared by set_output_len above.
        unsafe {
            host::set_output_byte(i as i32, byte as i32);
        }
    }
}

/// Set the error message that the host will read if `suture_merge` returns -1.
///
/// The message is stored in a static buffer. Only the last call's message is kept.
pub fn set_error(msg: &str) {
    // Log the error at Error level
    log(LogLevel::Error, msg);
    // Store in static buffer for the host to read via suture_error_msg
    ERROR_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        buf.extend_from_slice(msg.as_bytes());
    });
}

thread_local! {
    static ERROR_BUFFER: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------
// Merge input helpers
// ---------------------------------------------------------------------------

/// Parsed merge input from the host.
#[derive(Debug, Clone)]
pub struct MergeInput {
    pub base: String,
    pub ours: String,
    pub theirs: String,
}

impl MergeInput {
    /// Parse the JSON input from the host.
    pub fn from_host() -> Result<Self, String> {
        let input = read_input();
        let data: serde_json::Value =
            serde_json::from_str(&input).map_err(|e| format!("invalid JSON: {e}"))?;
        Ok(Self {
            base: data
                .get("base")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            ours: data
                .get("ours")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            theirs: data
                .get("theirs")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// Macro helpers for required exports
// ---------------------------------------------------------------------------

/// Declare the plugin metadata exports.
///
/// # Usage
///
/// ```rust,ignore
/// suture_plugin_sdk::declare_plugin!("my-plugin", "0.1.0", "json,yaml");
/// ```
#[macro_export]
macro_rules! declare_plugin {
    ($name:expr, $version:expr, $extensions:expr) => {
        /// ABI version — must be 1.
        #[no_mangle]
        pub extern "C" fn suture_abi_version() -> i32 {
            $crate::ABI_VERSION
        }

        #[no_mangle]
        pub extern "C" fn suture_plugin_name() -> *const u8 {
            concat!($name, "\0").as_ptr()
        }

        #[no_mangle]
        pub extern "C" fn suture_plugin_name_len() -> i32 {
            $name.len() as i32
        }

        #[no_mangle]
        pub extern "C" fn suture_plugin_version() -> *const u8 {
            concat!($version, "\0").as_ptr()
        }

        #[no_mangle]
        pub extern "C" fn suture_plugin_version_len() -> i32 {
            $version.len() as i32
        }

        #[no_mangle]
        pub extern "C" fn suture_extensions() -> *const u8 {
            concat!($extensions, "\0").as_ptr()
        }

        #[no_mangle]
        pub extern "C" fn suture_extensions_len() -> i32 {
            $extensions.len() as i32
        }

        #[no_mangle]
        pub extern "C" fn suture_error_msg() -> *const u8 {
            $crate::error_msg_ptr()
        }

        #[no_mangle]
        pub extern "C" fn suture_error_msg_len() -> i32 {
            $crate::error_msg_len()
        }
    };
}

/// Internal: returns pointer to the error message buffer.
#[doc(hidden)]
pub fn error_msg_ptr() -> *const u8 {
    ERROR_BUFFER.with(|buf| buf.borrow().as_ptr())
}

/// Internal: returns length of the error message buffer.
#[doc(hidden)]
pub fn error_msg_len() -> i32 {
    ERROR_BUFFER.with(|buf| buf.borrow().len() as i32)
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Common imports for plugin authors.
pub mod prelude {
    pub use crate::{log, read_input, set_error, write_output, LogLevel, MergeInput};
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub extensions: Vec<String>,
    pub author: String,
}

pub trait PluginDriver: Send + Sync {
    fn manifest(&self) -> PluginManifest;
    fn diff(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<PluginDiffResult, PluginError>;
    fn merge(
        &self,
        base: &[u8],
        ours: &[u8],
        theirs: &[u8],
    ) -> Result<PluginMergeResult, PluginError>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginDiffResult {
    pub changes: Vec<PluginChange>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginChange {
    pub path: String,
    pub description: String,
    pub severity: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginMergeResult {
    pub merged: Vec<u8>,
    pub conflicts: Vec<String>,
    pub clean: bool,
}

#[derive(Debug, Clone)]
pub enum PluginError {
    Runtime(String),
    UnsupportedFormat(String),
    Conflict(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::Runtime(msg) => write!(f, "plugin error: {msg}"),
            PluginError::UnsupportedFormat(fmt) => write!(f, "unsupported format: {fmt}"),
            PluginError::Conflict(msg) => write!(f, "merge conflict: {msg}"),
        }
    }
}

impl std::error::Error for PluginError {}

use std::sync::Arc;

pub struct PluginRegistry {
    drivers: std::collections::HashMap<String, Arc<dyn PluginDriver>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            drivers: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, driver: impl PluginDriver + 'static) {
        let driver: Arc<dyn PluginDriver> = Arc::new(driver);
        let manifest = driver.manifest();
        for ext in &manifest.extensions {
            self.drivers.insert(ext.clone(), Arc::clone(&driver));
        }
    }

    pub fn get_driver(&self, extension: &str) -> Option<&dyn PluginDriver> {
        self.drivers.get(extension).map(|d| d.as_ref())
    }

    pub fn list_plugins(&self) -> Vec<PluginManifest> {
        let mut seen = std::collections::HashSet::new();
        let mut manifests = Vec::new();
        for driver in self.drivers.values() {
            let m = driver.manifest();
            if seen.insert(m.name.clone()) {
                manifests.push(m);
            }
        }
        manifests
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests (these run on the host, not in WASM)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version_is_one() {
        assert_eq!(ABI_VERSION, 1);
    }

    #[test]
    fn test_log_level_values() {
        assert_eq!(LogLevel::Trace as i32, 0);
        assert_eq!(LogLevel::Debug as i32, 1);
        assert_eq!(LogLevel::Info as i32, 2);
        assert_eq!(LogLevel::Warn as i32, 3);
        assert_eq!(LogLevel::Error as i32, 4);
    }

    #[test]
    fn test_set_error_stores_message() {
        set_error("test error");
        ERROR_BUFFER.with(|buf| {
            let buf = buf.borrow();
            assert_eq!(buf.as_slice(), b"test error");
        });
    }

    #[test]
    fn test_error_msg_accessors() {
        set_error("hello");
        assert_eq!(error_msg_len(), 5);
    }

    #[test]
    fn test_plugin_registry_register_and_get() {
        struct MockDriver;
        impl PluginDriver for MockDriver {
            fn manifest(&self) -> PluginManifest {
                PluginManifest {
                    name: "test-plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: "test".to_string(),
                    extensions: vec!["json".to_string(), "yaml".to_string()],
                    author: "test".to_string(),
                }
            }
            fn diff(
                &self,
                _base: &[u8],
                _ours: &[u8],
                _theirs: &[u8],
            ) -> Result<PluginDiffResult, PluginError> {
                Ok(PluginDiffResult { changes: vec![] })
            }
            fn merge(
                &self,
                base: &[u8],
                ours: &[u8],
                theirs: &[u8],
            ) -> Result<PluginMergeResult, PluginError> {
                Ok(PluginMergeResult {
                    merged: base.to_vec(),
                    conflicts: vec![],
                    clean: ours == theirs,
                })
            }
        }

        let mut registry = PluginRegistry::new();
        registry.register(MockDriver);
        assert!(registry.get_driver("json").is_some());
        assert!(registry.get_driver("yaml").is_some());
        assert!(registry.get_driver("toml").is_none());
    }

    #[test]
    fn test_plugin_registry_list() {
        struct DriverA;
        impl PluginDriver for DriverA {
            fn manifest(&self) -> PluginManifest {
                PluginManifest {
                    name: "plugin-a".to_string(),
                    version: "1.0.0".to_string(),
                    description: "a".to_string(),
                    extensions: vec!["a".to_string()],
                    author: "test".to_string(),
                }
            }
            fn diff(
                &self,
                _base: &[u8],
                _ours: &[u8],
                _theirs: &[u8],
            ) -> Result<PluginDiffResult, PluginError> {
                Ok(PluginDiffResult { changes: vec![] })
            }
            fn merge(
                &self,
                _base: &[u8],
                _ours: &[u8],
                _theirs: &[u8],
            ) -> Result<PluginMergeResult, PluginError> {
                Ok(PluginMergeResult {
                    merged: vec![],
                    conflicts: vec![],
                    clean: true,
                })
            }
        }
        struct DriverB;
        impl PluginDriver for DriverB {
            fn manifest(&self) -> PluginManifest {
                PluginManifest {
                    name: "plugin-b".to_string(),
                    version: "2.0.0".to_string(),
                    description: "b".to_string(),
                    extensions: vec!["b".to_string()],
                    author: "test".to_string(),
                }
            }
            fn diff(
                &self,
                _base: &[u8],
                _ours: &[u8],
                _theirs: &[u8],
            ) -> Result<PluginDiffResult, PluginError> {
                Ok(PluginDiffResult { changes: vec![] })
            }
            fn merge(
                &self,
                _base: &[u8],
                _ours: &[u8],
                _theirs: &[u8],
            ) -> Result<PluginMergeResult, PluginError> {
                Ok(PluginMergeResult {
                    merged: vec![],
                    conflicts: vec![],
                    clean: true,
                })
            }
        }

        let mut registry = PluginRegistry::new();
        registry.register(DriverA);
        registry.register(DriverB);
        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 2);
        let names: std::collections::HashSet<&str> =
            plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains("plugin-a"));
        assert!(names.contains("plugin-b"));
    }

    #[test]
    fn test_plugin_manifest_serialization() {
        let manifest = PluginManifest {
            name: "serde-test".to_string(),
            version: "0.1.0".to_string(),
            description: "serialization test".to_string(),
            extensions: vec!["json".to_string(), "yaml".to_string()],
            author: "tester".to_string(),
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "serde-test");
        assert_eq!(deserialized.version, "0.1.0");
        assert_eq!(deserialized.extensions, vec!["json", "yaml"]);
        assert_eq!(deserialized.author, "tester");
    }
}
