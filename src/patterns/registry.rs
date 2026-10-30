//! Pattern registry for dynamic pattern management
//!
//! This module provides a centralized registry for pattern detectors that enables
//! dynamic registration, discovery, and execution of pattern detection algorithms.
//! The registry supports both sync and async pattern detectors with comprehensive
//! error handling and performance monitoring.

use crate::events::types::Candle;
use crate::patterns::traits::{PatternCategory, PatternDetector, PatternDetectorFactory};
use crate::patterns::types::{PatternError, PatternParams, PatternResult, PatternStats};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Thread-safe pattern registry for managing pattern detectors
///
/// The registry provides centralized management of pattern detectors with
/// support for dynamic registration, configuration, and execution. It uses
/// functional programming principles with immutable operations and pure functions.
///
/// # Features
///
/// - **Thread-Safe**: Uses DashMap for concurrent access
/// - **Dynamic Registration**: Add/remove patterns at runtime
/// - **Performance Monitoring**: Built-in statistics tracking
/// - **Category Filtering**: Filter patterns by category
/// - **Configuration Management**: Per-pattern parameter management
/// - **Error Handling**: Comprehensive error reporting
///
/// # Examples
///
/// ```
/// use daytrader::patterns::registry::PatternRegistry;
/// use daytrader::patterns::types::PatternParams;
///
/// let registry = PatternRegistry::new();
///
/// // Register patterns would be done here
/// // let detector = Box::new(HammerDetector::new());
/// // registry.register("hammer", detector);
///
/// // Detect patterns in candle data
/// // let results = registry.detect_all(&candles, &params).await;
/// ```
#[derive(Debug)]
pub struct PatternRegistry {
    /// Registered pattern detectors
    detectors: DashMap<String, Arc<dyn PatternDetector>>,
    /// Pattern-specific parameters
    pattern_params: DashMap<String, PatternParams>,
    /// Performance statistics per pattern
    stats: DashMap<String, PatternStats>,
    /// Pattern categories for filtering
    categories: DashMap<String, PatternCategory>,
    /// Global registry configuration
    config: RwLock<RegistryConfig>,
}

/// Configuration for the pattern registry
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Maximum number of patterns to run in parallel
    pub max_parallel_patterns: usize,
    /// Timeout for pattern detection in milliseconds
    pub detection_timeout_ms: u64,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
    /// Minimum confidence threshold for all patterns
    pub global_min_confidence: f64,
    /// Categories to enable (empty = all enabled)
    pub enabled_categories: Vec<PatternCategory>,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_parallel_patterns: 10,
            detection_timeout_ms: 1000,
            enable_monitoring: true,
            global_min_confidence: 0.0,
            enabled_categories: Vec::new(), // Empty means all enabled
        }
    }
}

impl PatternRegistry {
    /// Creates a new pattern registry with default configuration
    pub fn new() -> Self {
        Self {
            detectors: DashMap::new(),
            pattern_params: DashMap::new(),
            stats: DashMap::new(),
            categories: DashMap::new(),
            config: RwLock::new(RegistryConfig::default()),
        }
    }

    /// Creates a new pattern registry with custom configuration
    pub fn with_config(config: RegistryConfig) -> Self {
        Self {
            detectors: DashMap::new(),
            pattern_params: DashMap::new(),
            stats: DashMap::new(),
            categories: DashMap::new(),
            config: RwLock::new(config),
        }
    }

    /// Registers a pattern detector
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the pattern
    /// * `detector` - Pattern detector implementation
    ///
    /// # Errors
    ///
    /// Returns error if pattern name is already registered or detector is invalid.
    pub fn register(
        &self,
        name: impl Into<String>,
        detector: Box<dyn PatternDetector>,
    ) -> Result<(), PatternError> {
        let name = name.into();

        // Validate detector
        let default_params = detector.default_params();
        detector.validate_params(&default_params)?;

        // Check for duplicate registration
        if self.detectors.contains_key(&name) {
            return Err(PatternError::Configuration {
                error: format!("Pattern '{}' is already registered", name),
            });
        }

        let detector_arc: Arc<dyn PatternDetector> = Arc::from(detector);
        let category = detector_arc.category();

        // Register the detector
        self.detectors.insert(name.clone(), detector_arc);
        self.pattern_params.insert(name.clone(), default_params);
        self.stats.insert(name.clone(), PatternStats::new());
        self.categories.insert(name.clone(), category);

        info!("Registered pattern detector: {}", name);
        Ok(())
    }

    /// Registers a pattern detector from a factory
    pub fn register_from_factory(
        &self,
        factory: Box<dyn PatternDetectorFactory>,
    ) -> Result<(), PatternError> {
        let detector = factory.create();
        let name = factory.pattern_name();
        self.register(name, detector)
    }

    /// Unregisters a pattern detector
    pub fn unregister(&self, name: &str) -> Result<(), PatternError> {
        if !self.detectors.contains_key(name) {
            return Err(PatternError::Configuration {
                error: format!("Pattern '{}' is not registered", name),
            });
        }

        self.detectors.remove(name);
        self.pattern_params.remove(name);
        self.stats.remove(name);
        self.categories.remove(name);

        info!("Unregistered pattern detector: {}", name);
        Ok(())
    }

    /// Updates parameters for a specific pattern
    pub fn update_pattern_params(
        &self,
        name: &str,
        params: PatternParams,
    ) -> Result<(), PatternError> {
        let detector = self
            .detectors
            .get(name)
            .ok_or_else(|| PatternError::Configuration {
                error: format!("Pattern '{}' is not registered", name),
            })?;

        // Validate parameters with the detector
        detector.validate_params(&params)?;

        // Update parameters
        self.pattern_params.insert(name.to_string(), params);
        debug!("Updated parameters for pattern: {}", name);
        Ok(())
    }

    /// Gets parameters for a specific pattern
    pub fn get_pattern_params(&self, name: &str) -> Option<PatternParams> {
        self.pattern_params.get(name).map(|p| p.clone())
    }

    /// Detects patterns using a specific detector
    ///
    /// # Arguments
    ///
    /// * `pattern_name` - Name of the pattern to detect
    /// * `candles` - Candle data to analyze
    /// * `params` - Optional custom parameters (uses registered params if None)
    ///
    /// # Returns
    ///
    /// Pattern detection result or None if pattern not detected
    pub async fn detect_pattern(
        &self,
        pattern_name: &str,
        candles: &[Candle],
        params: Option<&PatternParams>,
    ) -> Result<Option<PatternResult>, PatternError> {
        let detector =
            self.detectors
                .get(pattern_name)
                .ok_or_else(|| PatternError::Configuration {
                    error: format!("Pattern '{}' is not registered", pattern_name),
                })?;

        // Get parameters (custom or registered)
        let detection_params = match params {
            Some(p) => p.clone(),
            None => self.get_pattern_params(pattern_name).unwrap_or_default(),
        };

        // Check if pattern category is enabled
        if !self.is_category_enabled(&detector.category()).await {
            debug!("Pattern '{}' category is disabled", pattern_name);
            return Ok(None);
        }

        // Validate minimum candles
        if candles.len() < detector.min_candles_required() {
            return Err(PatternError::InsufficientData {
                required: detector.min_candles_required(),
                available: candles.len(),
            });
        }

        // Perform detection with timing
        let start_time = Instant::now();
        let result = detector.detect(candles, &detection_params);
        let detection_time = start_time.elapsed().as_secs_f64() * 1000.0;

        // Record statistics if monitoring is enabled
        if self.is_monitoring_enabled().await {
            let confidence = result
                .as_ref()
                .ok()
                .and_then(|r| r.as_ref())
                .map(|r| r.confidence.value())
                .unwrap_or(0.0);

            if let Some(mut stats) = self.stats.get_mut(pattern_name) {
                stats.record_detection(pattern_name, confidence, detection_time);
            }
        }

        // Apply global confidence filter
        match result? {
            Some(pattern_result) => {
                let global_min_confidence = self.config.read().await.global_min_confidence;
                if pattern_result.confidence.value() >= global_min_confidence {
                    debug!(
                        "Pattern '{}' detected with confidence {:.2}",
                        pattern_name,
                        pattern_result.confidence.value()
                    );
                    Ok(Some(pattern_result))
                } else {
                    debug!(
                        "Pattern '{}' confidence {:.2} below global threshold {:.2}",
                        pattern_name,
                        pattern_result.confidence.value(),
                        global_min_confidence
                    );
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Detects patterns using all registered detectors
    ///
    /// Runs pattern detection in parallel for all enabled patterns.
    /// Returns a vector of all detected patterns sorted by confidence.
    ///
    /// `candles` MUST be ordered newest-first: detectors evaluate
    /// `candles[0]` as the current bar and treat `candles[1..]` as history
    /// (e.g. morning star reads `candles[2]` as the oldest of its three).
    /// Callers holding chronological data must pass a reversed view.
    pub async fn detect_all(
        &self,
        candles: &[Candle],
        global_params: Option<&PatternParams>,
    ) -> Vec<PatternResult> {
        let pattern_names: Vec<String> = self
            .detectors
            .iter()
            .map(|entry| entry.key().clone())
            .collect();

        let mut results = Vec::new();
        let max_parallel = self.config.read().await.max_parallel_patterns;

        // Process patterns in batches to limit parallelism
        for chunk in pattern_names.chunks(max_parallel) {
            let mut handles = Vec::new();

            for pattern_name in chunk {
                let pattern_name = pattern_name.clone();
                let candles = candles.to_vec(); // Clone for async
                let params = global_params.cloned();

                // Clone detector and params for async execution
                let detector = match self.detectors.get(&pattern_name) {
                    Some(d) => d.clone(),
                    None => continue,
                };

                let detection_params = match params.as_ref() {
                    Some(p) => p.clone(),
                    None => self.get_pattern_params(&pattern_name).unwrap_or_default(),
                };

                let global_min_confidence = self.config.read().await.global_min_confidence;
                let category = detector.category();
                let is_enabled = self.is_category_enabled(&category).await;

                let handle = tokio::spawn(async move {
                    if !is_enabled {
                        return None;
                    }

                    if candles.len() < detector.min_candles_required() {
                        return None;
                    }

                    match detector.detect(&candles, &detection_params) {
                        Ok(Some(result)) if result.confidence.value() >= global_min_confidence => {
                            Some(result)
                        }
                        Ok(_) => None,
                        Err(e) => {
                            warn!("Pattern detection failed for '{}': {}", pattern_name, e);
                            None
                        }
                    }
                });
                handles.push(handle);
            }

            // Collect results from this batch
            for handle in handles {
                if let Ok(Some(result)) = handle.await {
                    results.push(result);
                }
            }
        }

        // Sort by confidence (highest first)
        results.sort_by(|a, b| {
            b.confidence
                .value()
                .partial_cmp(&a.confidence.value())
                .unwrap()
        });

        info!(
            "Detected {} patterns from {} registered detectors",
            results.len(),
            pattern_names.len()
        );
        results
    }

    /// Detects patterns by category
    ///
    /// Only runs detectors in the specified category.
    pub async fn detect_by_category(
        &self,
        candles: &[Candle],
        category: PatternCategory,
        global_params: Option<&PatternParams>,
    ) -> Vec<PatternResult> {
        let pattern_names: Vec<String> = self
            .categories
            .iter()
            .filter(|entry| *entry.value() == category)
            .map(|entry| entry.key().clone())
            .collect();

        let mut results = Vec::new();

        for pattern_name in pattern_names {
            match self
                .detect_pattern(&pattern_name, candles, global_params)
                .await
            {
                Ok(Some(result)) => results.push(result),
                Ok(None) => {}
                Err(e) => warn!("Pattern detection failed for '{}': {}", pattern_name, e),
            }
        }

        // Sort by confidence
        results.sort_by(|a, b| {
            b.confidence
                .value()
                .partial_cmp(&a.confidence.value())
                .unwrap()
        });
        results
    }

    /// Returns statistics for all registered patterns
    pub fn get_all_stats(&self) -> HashMap<String, PatternStats> {
        self.stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Returns statistics for a specific pattern
    pub fn get_pattern_stats(&self, name: &str) -> Option<PatternStats> {
        self.stats.get(name).map(|s| s.clone())
    }

    /// Resets statistics for all patterns
    pub fn reset_all_stats(&self) {
        for mut entry in self.stats.iter_mut() {
            *entry.value_mut() = PatternStats::new();
        }
        info!("Reset statistics for all patterns");
    }

    /// Returns list of all registered pattern names
    pub fn list_patterns(&self) -> Vec<String> {
        self.detectors
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Returns list of patterns by category
    pub fn list_patterns_by_category(&self, category: PatternCategory) -> Vec<String> {
        self.categories
            .iter()
            .filter(|entry| *entry.value() == category)
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Checks if a pattern is registered
    pub fn is_registered(&self, name: &str) -> bool {
        self.detectors.contains_key(name)
    }

    /// Returns the number of registered patterns
    pub fn pattern_count(&self) -> usize {
        self.detectors.len()
    }

    /// Updates the registry configuration
    pub async fn update_config(&self, config: RegistryConfig) {
        *self.config.write().await = config;
        info!("Updated pattern registry configuration");
    }

    /// Gets the current registry configuration
    pub async fn get_config(&self) -> RegistryConfig {
        self.config.read().await.clone()
    }

    /// Checks if monitoring is enabled
    async fn is_monitoring_enabled(&self) -> bool {
        self.config.read().await.enable_monitoring
    }

    /// Checks if a category is enabled
    async fn is_category_enabled(&self, category: &PatternCategory) -> bool {
        let config = self.config.read().await;
        config.enabled_categories.is_empty() || config.enabled_categories.contains(category)
    }

    /// Validates all registered patterns
    ///
    /// Useful for startup validation to ensure all patterns are properly configured.
    pub async fn validate_all(&self) -> Result<(), PatternError> {
        let mut errors = Vec::new();

        for entry in self.detectors.iter() {
            let name = entry.key();
            let detector = entry.value();

            // Validate default parameters
            if let Err(e) = detector.validate_params(&detector.default_params()) {
                errors.push(format!("Pattern '{}': {}", name, e));
            }

            // Validate registered parameters
            if let Some(params) = self.get_pattern_params(name) {
                if let Err(e) = detector.validate_params(&params) {
                    errors.push(format!("Pattern '{}' params: {}", name, e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(PatternError::Configuration {
                error: format!("Validation errors: {}", errors.join(", ")),
            });
        }

        info!(
            "Validated {} pattern detectors successfully",
            self.pattern_count()
        );
        Ok(())
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating and configuring a pattern registry
#[derive(Debug)]
pub struct PatternRegistryBuilder {
    config: RegistryConfig,
    factories: Vec<Box<dyn PatternDetectorFactory>>,
}

impl PatternRegistryBuilder {
    /// Creates a new registry builder
    pub fn new() -> Self {
        Self {
            config: RegistryConfig::default(),
            factories: Vec::new(),
        }
    }

    /// Sets the maximum number of parallel patterns
    pub fn max_parallel_patterns(mut self, max: usize) -> Self {
        self.config.max_parallel_patterns = max;
        self
    }

    /// Sets the detection timeout
    pub fn detection_timeout_ms(mut self, timeout: u64) -> Self {
        self.config.detection_timeout_ms = timeout;
        self
    }

    /// Enables or disables monitoring
    pub fn enable_monitoring(mut self, enable: bool) -> Self {
        self.config.enable_monitoring = enable;
        self
    }

    /// Sets the global minimum confidence threshold
    pub fn global_min_confidence(mut self, confidence: f64) -> Self {
        self.config.global_min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Sets enabled categories
    pub fn enabled_categories(mut self, categories: Vec<PatternCategory>) -> Self {
        self.config.enabled_categories = categories;
        self
    }

    /// Adds a pattern detector factory
    pub fn add_factory(mut self, factory: Box<dyn PatternDetectorFactory>) -> Self {
        self.factories.push(factory);
        self
    }

    /// Builds the pattern registry
    pub async fn build(self) -> Result<PatternRegistry, PatternError> {
        let registry = PatternRegistry::with_config(self.config);

        // Register all factories
        for factory in self.factories {
            registry.register_from_factory(factory)?;
        }

        // Validate all patterns
        registry.validate_all().await?;

        Ok(registry)
    }
}

impl Default for PatternRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Confidence, PatternName, Price, Symbol};
    use crate::events::types::{Candle, SignalType};
    use crate::patterns::types::PatternResult;
    use chrono::Utc;
    use rust_decimal::Decimal;

    // Mock pattern detector for testing
    #[derive(Debug)]
    struct MockDetector {
        name: &'static str,
        should_detect: bool,
        confidence: f64,
    }

    impl PatternDetector for MockDetector {
        fn name(&self) -> &'static str {
            self.name
        }

        fn min_candles_required(&self) -> usize {
            1
        }

        fn detect(
            &self,
            candles: &[Candle],
            _params: &PatternParams,
        ) -> Result<Option<PatternResult>, PatternError> {
            if !self.should_detect {
                return Ok(None);
            }

            let result = PatternResult::new(
                PatternName::new(self.name),
                SignalType::Buy,
                Confidence::new(self.confidence).unwrap(),
                candles[0].close,
            );
            Ok(Some(result))
        }
    }

    fn create_test_candle() -> Candle {
        Candle {
            symbol: Symbol::new("TEST").unwrap(),
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(10000, 2)).unwrap(),
            high: Price::new(Decimal::new(11000, 2)).unwrap(),
            low: Price::new(Decimal::new(9000, 2)).unwrap(),
            close: Price::new(Decimal::new(10500, 2)).unwrap(),
            volume: 1000,
        }
    }

    #[tokio::test]
    async fn test_registry_registration() {
        let registry = PatternRegistry::new();

        let detector = Box::new(MockDetector {
            name: "test_pattern",
            should_detect: true,
            confidence: 0.8,
        });

        assert!(registry.register("test_pattern", detector).is_ok());
        assert!(registry.is_registered("test_pattern"));
        assert_eq!(registry.pattern_count(), 1);

        // Test duplicate registration
        let duplicate = Box::new(MockDetector {
            name: "test_pattern",
            should_detect: true,
            confidence: 0.8,
        });
        assert!(registry.register("test_pattern", duplicate).is_err());
    }

    #[tokio::test]
    async fn test_pattern_detection() {
        let registry = PatternRegistry::new();

        let detector = Box::new(MockDetector {
            name: "test_pattern",
            should_detect: true,
            confidence: 0.8,
        });
        registry.register("test_pattern", detector).unwrap();

        let candles = vec![create_test_candle()];
        let result = registry
            .detect_pattern("test_pattern", &candles, None)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_detect_all() {
        let registry = PatternRegistry::new();

        // Register multiple detectors
        let detector1 = Box::new(MockDetector {
            name: "pattern1",
            should_detect: true,
            confidence: 0.9,
        });
        let detector2 = Box::new(MockDetector {
            name: "pattern2",
            should_detect: true,
            confidence: 0.7,
        });
        let detector3 = Box::new(MockDetector {
            name: "pattern3",
            should_detect: false,
            confidence: 0.5,
        });

        registry.register("pattern1", detector1).unwrap();
        registry.register("pattern2", detector2).unwrap();
        registry.register("pattern3", detector3).unwrap();

        let candles = vec![create_test_candle()];
        let results = registry.detect_all(&candles, None).await;

        // Should detect 2 patterns (pattern3 doesn't detect)
        assert_eq!(results.len(), 2);
        // Should be sorted by confidence (pattern1 first with 0.9)
        assert_eq!(results[0].pattern_name.as_str(), "pattern1");
        assert_eq!(results[1].pattern_name.as_str(), "pattern2");
    }

    #[tokio::test]
    async fn test_global_confidence_filter() {
        let mut config = RegistryConfig::default();
        config.global_min_confidence = 0.8;
        let registry = PatternRegistry::with_config(config);

        let detector = Box::new(MockDetector {
            name: "low_confidence",
            should_detect: true,
            confidence: 0.7, // Below global threshold
        });
        registry.register("low_confidence", detector).unwrap();

        let candles = vec![create_test_candle()];
        let result = registry
            .detect_pattern("low_confidence", &candles, None)
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // Filtered out by global threshold
    }

    #[tokio::test]
    async fn test_pattern_params() {
        let registry = PatternRegistry::new();

        let detector = Box::new(MockDetector {
            name: "test_pattern",
            should_detect: true,
            confidence: 0.8,
        });
        registry.register("test_pattern", detector).unwrap();

        // Update parameters
        let custom_params = PatternParams::new().with_min_confidence(0.9);
        assert!(
            registry
                .update_pattern_params("test_pattern", custom_params.clone())
                .is_ok()
        );

        // Verify parameters were updated
        let retrieved_params = registry.get_pattern_params("test_pattern");
        assert!(retrieved_params.is_some());
        assert_eq!(retrieved_params.unwrap().min_confidence, 0.9);
    }

    #[tokio::test]
    async fn test_registry_builder() {
        let registry = PatternRegistryBuilder::new()
            .max_parallel_patterns(5)
            .global_min_confidence(0.7)
            .enable_monitoring(true)
            .build()
            .await;

        assert!(registry.is_ok());
        let registry = registry.unwrap();

        let config = registry.get_config().await;
        assert_eq!(config.max_parallel_patterns, 5);
        assert_eq!(config.global_min_confidence, 0.7);
        assert!(config.enable_monitoring);
    }
}
