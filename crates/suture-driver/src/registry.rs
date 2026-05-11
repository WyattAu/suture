use std::collections::HashMap;
use std::path::Path;

use crate::{DriverError, MergeStrategy, SutureDriver};

/// Registry of file format drivers, dispatching by file extension.
pub struct DriverRegistry {
    extension_map: HashMap<String, String>,
    drivers: HashMap<String, Box<dyn SutureDriver>>,
    strategies: Vec<Box<dyn MergeStrategy>>,
}

impl DriverRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            extension_map: HashMap::new(),
            drivers: HashMap::new(),
            strategies: Vec::new(),
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

    /// Register a custom merge strategy for specific file patterns.
    pub fn register_strategy(&mut self, strategy: Box<dyn MergeStrategy>) {
        self.strategies.push(strategy);
    }

    /// Get the first registered merge strategy that matches the given file path.
    #[must_use]
    pub fn get_strategy_for(&self, path: &str) -> Option<&dyn MergeStrategy> {
        self.strategies
            .iter()
            .find(|s| s.matches_path(path))
            .map(|s| s.as_ref())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MergeStrategyError, MergeStrategyResult};

    struct TestStrategy {
        strategy_name: &'static str,
        strategy_patterns: Vec<&'static str>,
    }

    impl TestStrategy {
        fn new(name: &'static str, patterns: Vec<&'static str>) -> Self {
            Self {
                strategy_name: name,
                strategy_patterns: patterns,
            }
        }
    }

    impl MergeStrategy for TestStrategy {
        fn name(&self) -> &str {
            self.strategy_name
        }
        fn file_patterns(&self) -> &[&str] {
            &self.strategy_patterns
        }
        fn merge(
            &self,
            _base: &[u8],
            ours: &[u8],
            _theirs: &[u8],
        ) -> Result<MergeStrategyResult, MergeStrategyError> {
            Ok(MergeStrategyResult {
                content: ours.to_vec(),
                had_conflicts: false,
            })
        }
    }

    #[test]
    fn register_and_retrieve_strategy() {
        let mut registry = DriverRegistry::new();
        registry.register_strategy(Box::new(TestStrategy::new("test", vec!["*.lock"])));
        assert!(registry.get_strategy_for("Cargo.lock").is_some());
        assert!(registry.get_strategy_for("other.txt").is_none());
    }

    #[test]
    fn get_strategy_returns_first_match() {
        let mut registry = DriverRegistry::new();
        registry.register_strategy(Box::new(TestStrategy::new("first", vec!["*.lock"])));
        registry.register_strategy(Box::new(TestStrategy::new("second", vec!["Cargo.lock"])));
        let strategy = registry.get_strategy_for("Cargo.lock").unwrap();
        assert_eq!(strategy.name(), "first");
    }

    #[test]
    fn no_strategy_registered() {
        let registry = DriverRegistry::new();
        assert!(registry.get_strategy_for("anything").is_none());
    }

    #[test]
    fn registry_with_drivers_and_strategies() {
        let mut registry = DriverRegistry::new();
        registry.register_strategy(Box::new(TestStrategy::new("test", vec!["Makefile"])));
        assert!(registry.get_strategy_for("Makefile").is_some());
        assert!(registry.drivers.is_empty());
    }
}
