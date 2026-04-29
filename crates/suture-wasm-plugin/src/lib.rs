//! Wasm plugin system for custom merge strategies.
//!
//! Plugins are compiled to Wasm and loaded at runtime. They implement
//! a simple interface: `merge(base, ours, theirs) -> Option<result>`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasmtime::{Engine, Linker, Module, Store};

/// Plugin ABI version — plugins must match this
pub const PLUGIN_ABI_VERSION: u32 = 1;

/// Maximum memory for a plugin (16 MB)
const _: u64 = 16 * 1024 * 1024;

/// Maximum execution time for a plugin merge (5 seconds)
const _: u64 = 5;

/// A loaded Wasm plugin that can merge files
pub struct WasmPlugin {
    engine: Engine,
    module: Module,
    name: String,
    driver_name: String,
    extensions: Vec<String>,
}

/// Result from a plugin merge operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMergeResult {
    pub merged: Option<String>,
    pub conflicts: bool,
    pub error: Option<String>,
}

/// Metadata exported by a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub driver_name: String,
    pub version: String,
    pub abi_version: u32,
    pub extensions: Vec<String>,
    pub description: String,
}

impl WasmPlugin {
    /// Load a plugin from Wasm bytes
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

    /// Extract plugin metadata by calling the metadata() export
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

    /// Get the plugin's driver name
    pub fn driver_name(&self) -> &str {
        &self.driver_name
    }

    /// Get the plugin's supported extensions
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Get the plugin's name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Execute a 3-way merge using the plugin
    pub fn merge(
        &self,
        base: &str,
        ours: &str,
        theirs: &str,
    ) -> Result<PluginMergeResult> {
        let mut store = Store::new(&self.engine, ());

        let mut linker = Linker::new(&self.engine);

        let plugin_name = self.name.clone();
        linker.func_wrap("env", "host_log", move |_caller: wasmtime::Caller<'_, ()>, level_ptr: i32, msg_ptr: i32| {
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

/// Plugin manager — loads and manages multiple plugins
pub struct PluginManager {
    plugins: Vec<Arc<WasmPlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Load a plugin from Wasm bytes
    pub fn load(&mut self, name: &str, wasm_bytes: &[u8]) -> Result<()> {
        let plugin = WasmPlugin::from_bytes(name, wasm_bytes)?;
        self.plugins.push(Arc::new(plugin));
        Ok(())
    }

    /// Load a plugin from a .wasm file
    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let wasm_bytes = std::fs::read(path)
            .with_context(|| format!("failed to read plugin file: {}", path))?;
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        self.load(name, &wasm_bytes)
    }

    /// Get a plugin by driver name
    pub fn get(&self, driver_name: &str) -> Option<&Arc<WasmPlugin>> {
        self.plugins.iter().find(|p| p.driver_name == driver_name)
    }

    /// List all loaded plugins
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| PluginInfo {
            name: p.name.clone(),
            driver_name: p.driver_name.clone(),
            extensions: p.extensions.clone(),
        }).collect()
    }

    /// Get total number of loaded plugins
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Info about a loaded plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub driver_name: String,
    pub extensions: Vec<String>,
}

/// Validate a Wasm module without fully loading it
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
