use std::collections::HashMap;
use std::path::Path;

use crate::{DriverError, SutureDriver};

/// Registry of file format drivers, dispatching by file extension.
pub struct DriverRegistry {
    extension_map: HashMap<String, String>,
    drivers: HashMap<String, Box<dyn SutureDriver>>,
}

impl DriverRegistry {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            extension_map: HashMap::new(),
            drivers: HashMap::new(),
        }
    }

    /// Register a driver for its supported extensions.
    pub fn register(&mut self, driver: Box<dyn SutureDriver>) {
        let name = driver.name().to_owned();
        for ext in driver.supported_extensions() {
            self.extension_map.insert(ext.to_lowercase(), name.clone());
        }
        self.drivers.insert(name, driver);
    }

    /// Get a driver for the given file path (by extension).
    pub fn get_for_path(&self, path: &Path) -> Result<&dyn SutureDriver, DriverError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e.to_lowercase()))
            .ok_or_else(|| DriverError::UnsupportedExtension(path.to_string_lossy().to_string()))?;

        self.get(&ext)
    }

    /// Get a driver for a specific extension string (e.g., ".json").
    pub fn get(&self, extension: &str) -> Result<&dyn SutureDriver, DriverError> {
        let ext = extension.to_lowercase();

        let driver_name = self
            .extension_map
            .get(&ext)
            .ok_or_else(|| DriverError::DriverNotFound(ext.clone()))?;

        self.drivers
            .get(driver_name)
            .map(std::convert::AsRef::as_ref)
            .ok_or(DriverError::DriverNotFound(ext))
    }

    /// List all registered drivers with their extensions.
    #[must_use] 
    pub fn list(&self) -> Vec<(&str, Vec<&str>)> {
        let mut result: Vec<(&str, Vec<&str>)> = self
            .drivers
            .values()
            .map(|d| {
                let exts: Vec<&str> = d.supported_extensions().to_vec();
                (d.name(), exts)
            })
            .collect();
        result.sort_by_key(|(name, _)| *name);
        result
    }
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}
