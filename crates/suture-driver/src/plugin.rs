use crate::SutureDriver;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to load plugin: {0}")]
    LoadFailed(String),
    #[error("missing required export: {0}")]
    MissingExport(String),
    #[error("plugin ABI version mismatch: expected {expected}, got {actual}")]
    AbiVersionMismatch { expected: i32, actual: i32 },
    #[cfg(feature = "wasm-plugins")]
    #[error("wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
}

pub trait DriverPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn description(&self) -> &str;
    fn as_driver(&self) -> &dyn SutureDriver;
}

pub struct BuiltinDriverPlugin<D> {
    name: &'static str,
    extensions: Vec<&'static str>,
    description: &'static str,
    driver: D,
}

impl<D: SutureDriver + Send + Sync + 'static> BuiltinDriverPlugin<D> {
    pub fn new(
        name: &'static str,
        extensions: Vec<&'static str>,
        description: &'static str,
        driver: D,
    ) -> Self {
        Self {
            name,
            extensions,
            description,
            driver,
        }
    }
}

impl<D: SutureDriver + Send + Sync + 'static> DriverPlugin for BuiltinDriverPlugin<D> {
    fn name(&self) -> &str {
        self.name
    }
    fn extensions(&self) -> &[&str] {
        &self.extensions
    }
    fn description(&self) -> &str {
        self.description
    }
    fn as_driver(&self) -> &dyn SutureDriver {
        &self.driver
    }
}

pub struct PluginRegistry {
    plugins: HashMap<String, Arc<dyn DriverPlugin>>,
    extension_map: HashMap<String, String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Arc<dyn DriverPlugin>) {
        let name = plugin.name().to_string();
        for ext in plugin.extensions() {
            self.extension_map.insert(ext.to_string(), name.clone());
        }
        self.plugins.insert(name, plugin);
    }

    pub fn get(&self, name: &str) -> Option<&dyn DriverPlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }

    pub fn get_by_extension(&self, ext: &str) -> Option<&dyn DriverPlugin> {
        let normalized = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{}", ext)
        };
        self.extension_map
            .get(&normalized)
            .and_then(|name| self.plugins.get(name).map(|p| p.as_ref()))
    }

    pub fn list_drivers(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.plugins.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn discover_plugins(&mut self, plugin_dir: &Path) {
        if !plugin_dir.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map(|e| e == "suture-plugin")
                    .unwrap_or(false)
                    && let Ok(content) = std::fs::read_to_string(&path)
                    && let Some(desc) = Self::parse_plugin_descriptor(&content)
                {
                    let _ = desc; // plugin descriptor found; future: dynamic loading
                }
            }
        }
    }

    fn parse_plugin_descriptor(content: &str) -> Option<PluginDescriptor> {
        let mut name = None;
        let mut extensions = Vec::new();
        let mut description = String::new();

        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line
                .strip_prefix("name")
                .and_then(Self::extract_string_value)
            {
                name = Some(val);
            } else if let Some(start) = line.find('[') {
                if let Some(end) = line[start..].find(']') {
                    let inner = &line[start + 1..start + end];
                    for ext in inner.split(',') {
                        let ext = ext.trim().trim_matches('"');
                        if !ext.is_empty() {
                            extensions.push(ext.to_string());
                        }
                    }
                }
            } else if let Some(val) = line
                .strip_prefix("description")
                .and_then(Self::extract_string_value)
            {
                description = val;
            }
        }

        name.map(|name| PluginDescriptor {
            name,
            extensions,
            description,
        })
    }

    fn extract_string_value(line: &str) -> Option<String> {
        if let Some(eq_pos) = line.find('=') {
            let val = line[eq_pos + 1..].trim();
            if val.starts_with('"') && val.ends_with('"') {
                return Some(val[1..val.len() - 1].to_string());
            }
        }
        None
    }
}

#[allow(dead_code)]
struct PluginDescriptor {
    name: String,
    extensions: Vec<String>,
    #[allow(dead_code)]
    description: String,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    #[cfg(feature = "wasm-plugins")]
    pub fn load_wasm_plugin(&mut self, path: &Path) -> Result<(), PluginError> {
        let plugin = WasmDriverPlugin::from_file(path)?;
        let name = plugin.name.clone();
        let extensions: Vec<String> = plugin.extensions_storage.clone();
        let plugin_arc = Arc::new(plugin);
        for ext in &extensions {
            self.extension_map.insert(ext.to_string(), name.clone());
        }
        self.plugins.insert(name, plugin_arc);
        Ok(())
    }
}

#[cfg(feature = "wasm-plugins")]
pub struct WasmDriverPlugin {
    #[allow(dead_code)]
    engine: wasmtime::Engine,
    #[allow(dead_code)]
    instance: wasmtime::Instance,
    #[allow(dead_code)]
    store: wasmtime::Store<()>,
    name: String,
    extensions_storage: Vec<String>,
    extensions: Vec<&'static str>,
}

#[cfg(feature = "wasm-plugins")]
impl WasmDriverPlugin {
    pub fn from_file(path: &std::path::Path) -> Result<Self, PluginError> {
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::from_file(&engine, path)
            .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

        let mut store = wasmtime::Store::new(&engine, ());
        let linker = wasmtime::Linker::new(&engine);

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

        let version = Self::call_version_export(&mut store, &instance)?;
        if version != 1 {
            return Err(PluginError::AbiVersionMismatch {
                expected: 1,
                actual: version,
            });
        }

        let name =
            Self::call_string_export(&mut store, &instance, "plugin_name")
                .unwrap_or_else(|| "unknown".to_string());
        let extensions_storage = Self::call_extensions_export(&mut store, &instance);

        let mut plugin = Self {
            engine,
            instance,
            store,
            name,
            extensions_storage,
            extensions: Vec::new(),
        };

        // SAFETY: extensions_storage is never modified after this point,
        // so the raw pointers remain valid for the lifetime of the struct.
        unsafe {
            plugin.extensions = plugin
                .extensions_storage
                .iter()
                .map(|s| {
                    let ptr: *const str = s.as_str();
                    &*ptr
                })
                .collect();
        }

        Ok(plugin)
    }

    fn call_version_export(
        store: &mut wasmtime::Store<()>,
        instance: &wasmtime::Instance,
    ) -> Result<i32, PluginError> {
        let func = instance
            .get_typed_func::<(), i32>(&mut *store, "plugin_version")
            .map_err(|_| PluginError::MissingExport("plugin_version".to_string()))?;
        let version = func.call(&mut *store, ())?;
        Ok(version)
    }

    fn call_string_export(
        store: &mut wasmtime::Store<()>,
        instance: &wasmtime::Instance,
        export_name: &str,
    ) -> Option<String> {
        let Ok(func) = instance.get_typed_func::<(), i32>(&mut *store, export_name) else {
            return None;
        };
        let Ok(ptr) = func.call(&mut *store, ()) else {
            return None;
        };

        let memory = instance
            .get_memory(&mut *store, "memory")?;

        let mut buf = Vec::new();
        let mut offset = ptr as usize;
        loop {
            if offset >= memory.data_size(&mut *store) {
                return None;
            }
            let byte = memory.data(&mut *store)[offset];
            if byte == 0 {
                break;
            }
            buf.push(byte);
            offset += 1;
        }
        String::from_utf8(buf).ok()
    }

    fn call_extensions_export(
        store: &mut wasmtime::Store<()>,
        instance: &wasmtime::Instance,
    ) -> Vec<String> {
        match Self::call_string_export(store, instance, "plugin_extensions") {
            Some(csv) => csv
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            None => vec![],
        }
    }
}

#[cfg(feature = "wasm-plugins")]
impl DriverPlugin for WasmDriverPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn extensions(&self) -> &[&str] {
        &self.extensions
    }

    fn description(&self) -> &str {
        "WASM plugin driver"
    }

    fn as_driver(&self) -> &dyn SutureDriver {
        self
    }
}

#[cfg(feature = "wasm-plugins")]
impl SutureDriver for WasmDriverPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_extensions(&self) -> &[&str] {
        &self.extensions
    }

    fn diff(
        &self,
        _base_content: Option<&str>,
        _new_content: &str,
    ) -> Result<Vec<crate::SemanticChange>, crate::DriverError> {
        Ok(vec![])
    }

    fn format_diff(
        &self,
        _base_content: Option<&str>,
        _new_content: &str,
    ) -> Result<String, crate::DriverError> {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DriverError;

    struct MockDriver {
        driver_name: &'static str,
        driver_extensions: Vec<&'static str>,
    }

    impl MockDriver {
        fn new(name: &'static str, extensions: Vec<&'static str>) -> Self {
            Self {
                driver_name: name,
                driver_extensions: extensions,
            }
        }
    }

    impl SutureDriver for MockDriver {
        fn name(&self) -> &str {
            self.driver_name
        }
        fn supported_extensions(&self) -> &[&str] {
            &self.driver_extensions
        }
        fn diff(
            &self,
            _base_content: Option<&str>,
            _new_content: &str,
        ) -> Result<Vec<crate::SemanticChange>, DriverError> {
            Ok(vec![])
        }
        fn format_diff(
            &self,
            _base_content: Option<&str>,
            _new_content: &str,
        ) -> Result<String, DriverError> {
            Ok(String::new())
        }
    }

    fn make_plugin(name: &'static str, extensions: Vec<&'static str>) -> Arc<dyn DriverPlugin> {
        Arc::new(BuiltinDriverPlugin::new(
            name,
            extensions.clone(),
            "test driver",
            MockDriver::new(name, extensions),
        ))
    }

    #[test]
    fn register_and_get_by_name() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("json", vec![".json"]));
        assert!(reg.get("json").is_some());
        assert!(reg.get("yaml").is_none());
        assert_eq!(reg.get("json").unwrap().name(), "json");
    }

    #[test]
    fn get_by_extension_with_dot() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("json", vec![".json"]));
        assert!(reg.get_by_extension(".json").is_some());
        assert!(reg.get_by_extension(".yaml").is_none());
    }

    #[test]
    fn get_by_extension_without_dot() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("yaml", vec![".yaml", ".yml"]));
        assert!(reg.get_by_extension("yaml").is_some());
        assert!(reg.get_by_extension("yml").is_some());
    }

    #[test]
    fn list_drivers_sorted() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("csv", vec![".csv"]));
        reg.register(make_plugin("xml", vec![".xml"]));
        reg.register(make_plugin("json", vec![".json"]));
        assert_eq!(reg.list_drivers(), vec!["csv", "json", "xml"]);
    }

    #[test]
    fn discover_plugins_nonexistent_dir() {
        let mut reg = PluginRegistry::new();
        reg.discover_plugins(Path::new("/tmp/suture-test-nonexistent-12345"));
        assert!(reg.list_drivers().is_empty());
    }

    #[test]
    fn parse_plugin_descriptor_valid() {
        let content = r#"
name = "my-driver"
extensions = [".custom", ".ext"]
description = "A custom driver"
"#;
        let desc = PluginRegistry::parse_plugin_descriptor(content).unwrap();
        assert_eq!(desc.name, "my-driver");
        assert_eq!(desc.extensions, vec![".custom", ".ext"]);
        assert_eq!(desc.description, "A custom driver");
    }

    #[test]
    fn parse_plugin_descriptor_missing_name() {
        let content = r#"extensions = [".custom"]"#;
        assert!(PluginRegistry::parse_plugin_descriptor(content).is_none());
    }

    #[test]
    fn as_driver_returns_underlying_driver() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("json", vec![".json"]));
        let plugin = reg.get("json").unwrap();
        assert_eq!(plugin.as_driver().name(), "json");
        assert_eq!(plugin.as_driver().supported_extensions(), &[".json"]);
    }

    #[cfg(feature = "wasm-plugins")]
    #[test]
    fn test_wasm_plugin_abi_documentation() {
        let abi_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/wasm_abi.md");
        let content = std::fs::read_to_string(&abi_path)
            .expect("wasm_abi.md should exist");
        assert!(content.contains("plugin_name"), "ABI doc should define plugin_name export");
        assert!(content.contains("plugin_extensions"), "ABI doc should define plugin_extensions export");
        assert!(content.contains("plugin_version"), "ABI doc should define plugin_version export");
        assert!(content.contains("merge"), "ABI doc should define merge function");
        assert!(content.contains("diff"), "ABI doc should define diff function");
        assert!(content.contains("ABI Version"), "ABI doc should specify version");
        assert!(content.contains("Memory Layout"), "ABI doc should specify memory layout");
        assert!(content.contains("Error Handling"), "ABI doc should specify error handling");
    }

    #[cfg(feature = "wasm-plugins")]
    #[test]
    fn test_plugin_registry_load_wasm_missing_file() {
        let mut reg = PluginRegistry::new();
        let result = reg.load_wasm_plugin(Path::new("/tmp/nonexistent-plugin.wasm"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::LoadFailed(msg) => {
                assert!(msg.contains("failed to read") || msg.contains("No such file"));
            }
            other => panic!("expected LoadFailed, got: {other}"),
        }
    }

    #[cfg(feature = "wasm-plugins")]
    #[test]
    fn test_plugin_registry_load_wasm_invalid_module() {
        let dir = std::env::temp_dir().join("suture-wasm-test-invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("invalid.wasm");
        std::fs::write(&path, b"not a valid wasm module").unwrap();

        let mut reg = PluginRegistry::new();
        let result = reg.load_wasm_plugin(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
