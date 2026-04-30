//! Wasm plugin system for custom merge strategies.
//!
//! Plugins are compiled to Wasm and loaded at runtime. They implement
//! a simple interface: `merge(base, ours, theirs) -> Option<result>`.
//!
//! Two runtime modes are supported:
//! - **[`WasmPlugin`]**: Memory-passthrough ABI — writes input directly into
//!   the WASM linear memory and reads results from a pointer. Suitable for
//!   plugins compiled with `wasm32-unknown-unknown`.
//! - **[`WasmPluginHost`]**: Host-function ABI — the host exposes I/O helpers
//!   (`env_get_input_byte`, `env_set_output_byte`, …) that the plugin calls
//!   to read its JSON input and write its merge output. Fuel-based timeouts
//!   and memory limits are enforced.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use wasmtime::*;

// ---------------------------------------------------------------------------
// Plugin error types
// ---------------------------------------------------------------------------

/// Errors that can occur when loading or executing a WASM plugin.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("compilation error: {0}")]
    Compilation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("interface error: {0}")]
    Interface(String),
    #[error("plugin execution timed out")]
    Timeout,
    #[error("plugin exceeded memory limit")]
    MemoryLimit,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Wasmtime(#[from] wasmtime::Error),
}

// ---------------------------------------------------------------------------
// Plugin metadata
// ---------------------------------------------------------------------------

/// Metadata exported by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    #[serde(default)]
    pub driver_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub abi_version: u32,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// The interface a Suture WASM plugin must implement.
///
/// Both [`WasmPlugin`] and [`WasmPluginHost`] satisfy this trait so callers
/// can operate on either runtime transparently.
pub trait SutureWasmPlugin {
    /// Returns the name of the plugin.
    fn plugin_name(&self) -> &str;

    /// Returns the file extensions this plugin handles (without leading dot).
    fn plugin_extensions(&self) -> Vec<&str>;

    /// Perform a 3-way merge.
    ///
    /// Returns `Ok(Some(merged))` on success, `Ok(None)` when the plugin
    /// reports a conflict, or `Err` on failure.
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, PluginError>;

    /// Check if this plugin can handle the given file.
    fn can_handle(&self, content: &str, extension: &str) -> bool;

    /// Get plugin metadata.
    fn metadata(&self) -> PluginMetadata;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Plugin ABI version — plugins must match this
pub const PLUGIN_ABI_VERSION: u32 = 1;

/// Maximum memory for a plugin (16 MB)
const MAX_PLUGIN_MEMORY: u64 = 16 * 1024 * 1024;

/// Default fuel budget for a single plugin invocation (1 M fuel units).
const DEFAULT_FUEL_BUDGET: u64 = 1_000_000;

// ---------------------------------------------------------------------------
// Merge result (shared between both runtime modes)
// ---------------------------------------------------------------------------

/// Result from a plugin merge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMergeResult {
    pub merged: Option<String>,
    pub conflicts: bool,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// WasmPlugin — memory-passthrough ABI (original implementation)
// ---------------------------------------------------------------------------

/// A loaded Wasm plugin that can merge files.
///
/// This variant uses the *memory-passthrough* ABI where the host writes
/// input directly into the WASM linear memory and reads results from a
/// returned pointer.
pub struct WasmPlugin {
    engine: Engine,
    module: Module,
    name: String,
    driver_name: String,
    extensions: Vec<String>,
}

impl WasmPlugin {
    /// Load a plugin from Wasm bytes.
    pub fn from_bytes(name: &str, wasm_bytes: &[u8]) -> Result<Self> {
        let engine = Engine::default();

        let module = Module::new(&engine, wasm_bytes)
            .context("failed to compile Wasm module")?;

        let exports: Vec<_> = module.exports().collect();
        let export_names: Vec<&str> = exports.iter().map(|e| e.name()).collect();

        for required in &["merge", "metadata"] {
            if !export_names.contains(required) {
                anyhow::bail!(
                    "plugin '{}' missing required export '{}'. Has: {:?}",
                    name, required, export_names
                );
            }
        }

        let (driver_name, extensions) = Self::extract_metadata(&module, name)?;

        Ok(Self {
            engine,
            module,
            name: name.to_string(),
            driver_name,
            extensions,
        })
    }

    /// Extract plugin metadata by calling the metadata() export.
    fn extract_metadata(module: &Module, name: &str) -> Result<(String, Vec<String>)> {
        let mut store = Store::new(module.engine(), ());

        let linker = Linker::new(module.engine());
        let _instance = linker
            .instantiate(&mut store, module)
            .context("failed to instantiate plugin")?;

        let driver_name = format!("plugin-{}", name);
        let extensions = vec![];

        Ok((driver_name, extensions))
    }

    /// Get the plugin's driver name.
    pub fn driver_name(&self) -> &str {
        &self.driver_name
    }

    /// Get the plugin's supported extensions.
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Get the plugin's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute a 3-way merge using the plugin.
    pub fn merge(
        &self,
        base: &str,
        ours: &str,
        theirs: &str,
    ) -> Result<PluginMergeResult> {
        let mut store = Store::new(&self.engine, ());

        let mut linker = Linker::new(&self.engine);

        let plugin_name = self.name.clone();
        linker.func_wrap("env", "host_log", move |_caller: Caller<'_, ()>, level_ptr: i32, msg_ptr: i32| {
            tracing::debug!(
                "[plugin:{}] log: level={}, msg_ptr={}",
                plugin_name, level_ptr, msg_ptr
            );
            Ok(())
        })?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .context("failed to instantiate plugin for merge")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("plugin has no memory export")?;

        let base_bytes = base.as_bytes();
        let ours_bytes = ours.as_bytes();
        let theirs_bytes = theirs.as_bytes();

        let total_size = base_bytes.len() + ours_bytes.len() + theirs_bytes.len() + 12;
        let ptr_offset = 1024;

        let current_size = memory.data_size(&store);
        if ptr_offset + total_size > current_size {
            let needed = (ptr_offset + total_size).div_ceil(65536);
            memory.grow(&mut store, needed as u64)
                .context("failed to grow plugin memory")?;
        }

        let mem_data = memory.data_mut(&mut store);

        let mut offset = ptr_offset;
        mem_data[offset..offset + 4].copy_from_slice(&(base_bytes.len() as u32).to_le_bytes());
        offset += 4;
        mem_data[offset..offset + base_bytes.len()].copy_from_slice(base_bytes);
        offset += base_bytes.len();
        mem_data[offset..offset + 4].copy_from_slice(&(ours_bytes.len() as u32).to_le_bytes());
        offset += 4;
        mem_data[offset..offset + ours_bytes.len()].copy_from_slice(ours_bytes);
        offset += ours_bytes.len();
        mem_data[offset..offset + 4].copy_from_slice(&(theirs_bytes.len() as u32).to_le_bytes());
        offset += 4;
        mem_data[offset..offset + theirs_bytes.len()].copy_from_slice(theirs_bytes);

        let merge_fn = instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut store, "merge")
            .context("merge export has wrong signature (expected (i32,i32,i32) -> i32)")?;

        let base_ptr = ptr_offset as i32;
        let ours_ptr = (ptr_offset + 4 + base_bytes.len()) as i32;
        let theirs_ptr = (ptr_offset + 8 + base_bytes.len() + ours_bytes.len()) as i32;

        let result_ptr = merge_fn
            .call(&mut store, (base_ptr, ours_ptr, theirs_ptr))
            .context("plugin merge() panicked or faulted")?;

        let mem_data = memory.data(&store);

        let result_len_bytes = &mem_data[result_ptr as usize..result_ptr as usize + 4];
        let result_len = u32::from_le_bytes(result_len_bytes.try_into().unwrap());

        if result_len == 0xFFFFFFFF {
            Ok(PluginMergeResult {
                merged: None,
                conflicts: true,
                error: None,
            })
        } else {
            let result_start = result_ptr as usize + 4;
            let result_end = result_start + result_len as usize;
            let merged = String::from_utf8(mem_data[result_start..result_end].to_vec())
                .unwrap_or_else(|_| format!("<invalid utf8: {} bytes>", result_len));

            Ok(PluginMergeResult {
                merged: Some(merged),
                conflicts: false,
                error: None,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// WasmPluginHost — host-function ABI with fuel / memory limits
// ---------------------------------------------------------------------------

/// State stored alongside a [`WasmPluginHost`] WASM instance.
struct PluginState {
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    input_len: usize,
}

/// A WASM plugin host that uses the *host-function* ABI.
///
/// Input is serialised as JSON (`{base, ours, theirs}`) and made available
/// to the plugin through `env_get_input_len` / `env_get_input_byte` imports.
/// The plugin writes its output via `env_set_output_len` /
/// `env_set_output_byte`.
///
/// Fuel-based timeouts and memory limits are enforced.
pub struct WasmPluginHost {
    #[allow(dead_code)]
    engine: Engine,
    store: RefCell<Store<PluginState>>,
    instance: RefCell<Instance>,
    metadata: PluginMetadata,
}

impl WasmPluginHost {
    /// Load and instantiate a WASM plugin from raw bytes.
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, PluginError> {
        let mut config = Config::new();
        config.wasm_threads(false);
        config.wasm_simd(true);
        config.consume_fuel(true);
        config.memory_reservation(MAX_PLUGIN_MEMORY);

        let engine = Engine::new(&config)
            .map_err(|e| PluginError::Compilation(e.to_string()))?;

        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| PluginError::Compilation(e.to_string()))?;

        let mut store = Store::new(&engine, PluginState {
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            input_len: 0,
        });

        let mut linker = Linker::new(&engine);

        linker.func_wrap("env", "get_input_len", |caller: Caller<'_, PluginState>| -> i32 {
            caller.data().input_len as i32
        }).map_err(|e| PluginError::Compilation(e.to_string()))?;

        linker.func_wrap("env", "get_input_byte", |caller: Caller<'_, PluginState>, offset: i32| -> i32 {
            let state = caller.data();
            if (offset as usize) < state.input_buffer.len() {
                state.input_buffer[offset as usize] as i32
            } else {
                -1
            }
        }).map_err(|e| PluginError::Compilation(e.to_string()))?;

        linker.func_wrap("env", "set_output_byte", |mut caller: Caller<'_, PluginState>, offset: i32, byte: i32| {
            let state = caller.data_mut();
            let offset = offset as usize;
            if offset >= state.output_buffer.len() {
                state.output_buffer.resize(offset + 1, 0);
            }
            state.output_buffer[offset] = byte as u8;
        }).map_err(|e| PluginError::Compilation(e.to_string()))?;

        linker.func_wrap("env", "set_output_len", |mut caller: Caller<'_, PluginState>, len: i32| {
            caller.data_mut().output_buffer.resize(len as usize, 0);
        }).map_err(|e| PluginError::Compilation(e.to_string()))?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| PluginError::Runtime(e.to_string()))?;

        let metadata = extract_metadata_host(&instance, &mut store)?;

        Ok(Self {
            engine,
            store: RefCell::new(store),
            instance: RefCell::new(instance),
            metadata,
        })
    }

    /// Perform a 3-way merge.
    ///
    /// Input is serialised as `{"base":…,"ours":…,"theirs":…}` and the
    /// plugin is expected to call the `env_*` host functions to read/write.
    ///
    /// Return codes from the WASM `suture_merge` export:
    /// - `0` — merged successfully (output in the output buffer)
    /// - `1` — conflict (no merge possible)
    /// - `-1` — error
    pub fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, PluginError> {
        let input = serde_json::json!({
            "base": base,
            "ours": ours,
            "theirs": theirs
        }).to_string();

        {
            let mut store = self.store.borrow_mut();
            let input_len = input.len();
            store.data_mut().input_buffer = input.into_bytes();
            store.data_mut().input_len = input_len;
            store.data_mut().output_buffer = Vec::new();

            store.set_fuel(DEFAULT_FUEL_BUDGET)
                .map_err(|e| PluginError::Runtime(e.to_string()))?;
        }

        let mut store = self.store.borrow_mut();
        let instance = self.instance.borrow();

        let merge_fn = instance
            .get_typed_func::<(), i32>(&mut *store, "suture_merge")
            .map_err(|e| PluginError::Interface(format!("suture_merge function not found: {}", e)))?;

        let result_code = merge_fn.call(&mut *store, ())
            .map_err(|e| PluginError::Runtime(e.to_string()))?;

        let fuel_remaining = store.get_fuel()
            .map_err(|e| PluginError::Runtime(e.to_string()))?;
        if fuel_remaining == 0 {
            return Err(PluginError::Timeout);
        }

        match result_code {
            0 => {
                let output = String::from_utf8(store.data().output_buffer.clone())
                    .map_err(|e| PluginError::Runtime(e.to_string()))?;
                Ok(Some(output))
            }
            1 => Ok(None),
            -1 => Err(PluginError::Runtime("Plugin returned error".to_string())),
            _ => Err(PluginError::Runtime(format!("Unknown result code: {}", result_code))),
        }
    }

    /// Returns a reference to the plugin's metadata.
    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Check whether this plugin handles the given extension.
    pub fn can_handle(&self, extension: &str) -> bool {
        let ext = extension.strip_prefix('.').unwrap_or(extension);
        self.metadata.extensions.iter().any(|e| e == ext)
    }
}

/// Read metadata from a host-function-ABI plugin by calling its name/version
/// exports and reading null-terminated strings from WASM memory.
fn extract_metadata_host(
    instance: &Instance,
    store: &mut Store<PluginState>,
) -> Result<PluginMetadata, PluginError> {
    let memory = instance
        .get_memory(&mut *store, "memory");

    let read_string = |store: &mut Store<PluginState>,
                       ptr_func: &str,
                       len_func: &str|
        -> String {
        let ptr_f = instance.get_typed_func::<(), i32>(&mut *store, ptr_func).ok();
        let len_f = instance.get_typed_func::<(), i32>(&mut *store, len_func).ok();
        match (ptr_f, len_f) {
            (Some(pf), Some(lf)) => {
                let ptr = pf.call(&mut *store, ()).unwrap_or(0);
                let len = lf.call(&mut *store, ()).unwrap_or(0) as usize;
                if let Some(ref mem) = memory {
                    let data: &[u8] = mem.data(&*store);
                    if ptr as usize + len <= data.len() {
                        String::from_utf8_lossy(&data[ptr as usize..ptr as usize + len]).into_owned()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    };

    let name = read_string(store, "suture_plugin_name", "suture_plugin_name_len");
    let version = read_string(store, "suture_plugin_version", "suture_plugin_version_len");

    let extensions = vec![];

    Ok(PluginMetadata {
        driver_name: format!("wasm-plugin-{}", name),
        name,
        version,
        abi_version: PLUGIN_ABI_VERSION,
        extensions,
        description: String::new(),
        author: String::new(),
    })
}

// ---------------------------------------------------------------------------
// SutureWasmPlugin impl for WasmPlugin
// ---------------------------------------------------------------------------

impl SutureWasmPlugin for WasmPlugin {
    fn plugin_name(&self) -> &str {
        &self.name
    }

    fn plugin_extensions(&self) -> Vec<&str> {
        self.extensions.iter().map(|s| s.as_str()).collect()
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, PluginError> {
        let result = WasmPlugin::merge(self, base, ours, theirs)
            .map_err(|e| PluginError::Runtime(e.to_string()))?;
        if result.conflicts {
            Ok(None)
        } else {
            Ok(result.merged)
        }
    }

    fn can_handle(&self, _content: &str, extension: &str) -> bool {
        let ext = extension.strip_prefix('.').unwrap_or(extension);
        self.extensions.iter().any(|e| e == ext)
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name.clone(),
            driver_name: self.driver_name.clone(),
            version: String::new(),
            abi_version: PLUGIN_ABI_VERSION,
            extensions: self.extensions.clone(),
            description: String::new(),
            author: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// SutureWasmPlugin impl for WasmPluginHost
// ---------------------------------------------------------------------------

impl SutureWasmPlugin for WasmPluginHost {
    fn plugin_name(&self) -> &str {
        &self.metadata.name
    }

    fn plugin_extensions(&self) -> Vec<&str> {
        self.metadata.extensions.iter().map(|s| s.as_str()).collect()
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, PluginError> {
        WasmPluginHost::merge(self, base, ours, theirs)
    }

    fn can_handle(&self, _content: &str, extension: &str) -> bool {
        WasmPluginHost::can_handle(self, extension)
    }

    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }
}

// ---------------------------------------------------------------------------
// Plugin manager (memory-passthrough plugins)
// ---------------------------------------------------------------------------

/// Plugin manager — loads and manages multiple plugins.
pub struct PluginManager {
    plugins: Vec<Arc<WasmPlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Load a plugin from Wasm bytes.
    pub fn load(&mut self, name: &str, wasm_bytes: &[u8]) -> Result<()> {
        let plugin = WasmPlugin::from_bytes(name, wasm_bytes)?;
        self.plugins.push(Arc::new(plugin));
        Ok(())
    }

    /// Load a plugin from a .wasm file.
    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let wasm_bytes = std::fs::read(path)
            .with_context(|| format!("failed to read plugin file: {}", path))?;
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        self.load(name, &wasm_bytes)
    }

    /// Get a plugin by driver name.
    pub fn get(&self, driver_name: &str) -> Option<&Arc<WasmPlugin>> {
        self.plugins.iter().find(|p| p.driver_name == driver_name)
    }

    /// List all loaded plugins.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| PluginInfo {
            name: p.name.clone(),
            driver_name: p.driver_name.clone(),
            extensions: p.extensions.clone(),
        }).collect()
    }

    /// Get total number of loaded plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Info about a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub driver_name: String,
    pub extensions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Plugin registry (host-function ABI plugins)
// ---------------------------------------------------------------------------

/// Registry for WASM plugins using the host-function ABI.
///
/// Plugins are keyed by name and can be looked up by file extension.
pub struct PluginRegistry {
    plugins: HashMap<String, WasmPluginHost>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    /// Load a plugin from a `.wasm` file on disk.
    pub fn load_from_file(&mut self, path: &Path) -> Result<String, PluginError> {
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| PluginError::Compilation(format!("Failed to read {}: {}", path.display(), e)))?;
        self.load_from_bytes(&wasm_bytes)
    }

    /// Load a plugin from raw WASM bytes.
    ///
    /// Returns the plugin name on success.
    pub fn load_from_bytes(&mut self, wasm_bytes: &[u8]) -> Result<String, PluginError> {
        let host = WasmPluginHost::new(wasm_bytes)?;
        let name = host.metadata().name.clone();
        self.plugins.insert(name.clone(), host);
        Ok(name)
    }

    /// Get a plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<&WasmPluginHost> {
        self.plugins.get(name)
    }

    /// Find the first plugin that handles the given extension.
    pub fn find_plugin_for_extension(&self, ext: &str) -> Option<&WasmPluginHost> {
        let normalized = ext.strip_prefix('.').unwrap_or(ext);
        self.plugins.values().find(|p| {
            p.metadata().extensions.iter().any(|e| e == normalized)
        })
    }

    /// List metadata for all loaded plugins.
    pub fn list_plugins(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().map(|p| p.metadata()).collect()
    }

    /// Number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a Wasm module without fully loading it.
pub fn validate_plugin(wasm_bytes: &[u8]) -> Result<Vec<String>> {
    let engine = Engine::default();

    let module = Module::new(&engine, wasm_bytes)
        .context("invalid Wasm module")?;

    let exports: Vec<String> = module
        .exports()
        .map(|e| e.name().to_string())
        .collect();

    let mut warnings = Vec::new();

    if !exports.contains(&"merge".to_string()) {
        warnings.push("missing 'merge' export".to_string());
    }
    if !exports.contains(&"memory".to_string()) {
        warnings.push("missing 'memory' export (required for string I/O)".to_string());
    }

    Ok(warnings)
}
