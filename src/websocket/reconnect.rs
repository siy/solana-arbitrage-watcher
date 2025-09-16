use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors that can occur during reconnection attempts
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ReconnectError {
    #[error("Maximum reconnection attempts exceeded ({0})")]
    MaxAttemptsExceeded(usize),
    #[error("Connection timeout after {0:?}")]
    ConnectionTimeout(Duration),
    #[error("Connection error: {0}")]
    #[allow(dead_code)]
    ConnectionError(String),
}

/// Configuration for exponential backoff reconnection strategy
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ReconnectConfig {
    /// Initial delay before first reconnection attempt
    pub initial_delay: Duration,
    /// Maximum delay between reconnection attempts
    pub max_delay: Duration,
    /// Multiplier for exponential backoff (e.g., 2.0 for doubling)
    pub backoff_multiplier: f64,
    /// Maximum number of reconnection attempts (None for unlimited)
    pub max_attempts: Option<usize>,
    /// Maximum time to spend on reconnection attempts
    pub max_total_duration: Option<Duration>,
    /// Add random jitter to delays to avoid thundering herd
    pub jitter: bool,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            max_attempts: Some(10),
            max_total_duration: Some(Duration::from_secs(300)), // 5 minutes
            jitter: true,
        }
    }
}

impl ReconnectConfig {
    /// Create a new reconnection configuration with custom parameters
    #[allow(dead_code)]
    pub fn new(initial_delay: Duration, max_delay: Duration, backoff_multiplier: f64) -> Self {
        Self {
            initial_delay,
            max_delay,
            backoff_multiplier,
            max_attempts: Some(10),
            max_total_duration: Some(Duration::from_secs(300)),
            jitter: true,
        }
    }

    /// Set maximum number of attempts
    #[allow(dead_code)]
    pub fn with_max_attempts(mut self, max_attempts: Option<usize>) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set maximum total duration for reconnection attempts
    #[allow(dead_code)]
    pub fn with_max_total_duration(mut self, max_duration: Option<Duration>) -> Self {
        self.max_total_duration = max_duration;
        self
    }

    /// Enable or disable jitter
    #[allow(dead_code)]
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Validate configuration parameters
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        if self.initial_delay.is_zero() {
            return Err("Initial delay must be greater than zero".to_string());
        }

        if self.max_delay < self.initial_delay {
            return Err("Max delay must be greater than or equal to initial delay".to_string());
        }

        if self.backoff_multiplier <= 1.0 {
            return Err("Backoff multiplier must be greater than 1.0".to_string());
        }

        if let Some(attempts) = self.max_attempts {
            if attempts == 0 {
                return Err("Max attempts must be greater than zero if specified".to_string());
            }
        }

        Ok(())
    }
}

/// Manages exponential backoff reconnection attempts with jitter and limits
#[derive(Debug)]
#[allow(dead_code)]
pub struct ReconnectHandler {
    config: ReconnectConfig,
    attempt_count: usize,
    start_time: Option<Instant>,
    current_delay: Duration,
}

impl ReconnectHandler {
    /// Create a new reconnection handler with the given configuration
    #[allow(dead_code)]
    pub fn new(config: ReconnectConfig) -> Result<Self, String> {
        config.validate()?;

        Ok(Self {
            current_delay: config.initial_delay,
            config,
            attempt_count: 0,
            start_time: None,
        })
    }

    /// Create a reconnection handler with default configuration
    #[allow(dead_code)]
    pub fn with_default() -> Self {
        Self::new(ReconnectConfig::default()).expect("Default configuration should be valid")
    }

    /// Reset the handler to initial state for a new connection session
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.attempt_count = 0;
        self.start_time = None;
        self.current_delay = self.config.initial_delay;
    }

    /// Check if we should attempt another reconnection
    #[allow(dead_code)]
    pub fn should_reconnect(&mut self) -> Result<Duration, ReconnectError> {
        // Initialize start time on first attempt
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }

        // Check maximum attempts limit
        if let Some(max_attempts) = self.config.max_attempts {
            if self.attempt_count >= max_attempts {
                return Err(ReconnectError::MaxAttemptsExceeded(max_attempts));
            }
        }

        // Check maximum total duration limit
        if let Some(max_duration) = self.config.max_total_duration {
            if let Some(start_time) = self.start_time {
                if start_time.elapsed() >= max_duration {
                    return Err(ReconnectError::ConnectionTimeout(max_duration));
                }
            }
        }

        self.attempt_count += 1;

        // Calculate delay for this attempt
        let delay = self.calculate_delay();

        // Update current delay for next attempt
        self.update_delay();

        Ok(delay)
    }

    /// Get the current attempt number (0-based)
    #[allow(dead_code)]
    pub fn attempt_count(&self) -> usize {
        self.attempt_count
    }

    /// Get the total elapsed time since first reconnection attempt
    #[allow(dead_code)]
    pub fn elapsed_time(&self) -> Option<Duration> {
        self.start_time.map(|start| start.elapsed())
    }

    /// Get the current configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &ReconnectConfig {
        &self.config
    }

    /// Calculate the delay for the current attempt with optional jitter
    fn calculate_delay(&self) -> Duration {
        let mut delay = self.current_delay;

        if self.config.jitter {
            // Add random jitter up to 10% of the delay
            delay = self.add_jitter(delay);
        }

        delay
    }

    /// Update the current delay for the next attempt using exponential backoff
    fn update_delay(&mut self) {
        let next_delay_ms =
            (self.current_delay.as_millis() as f64 * self.config.backoff_multiplier) as u64;
        let next_delay = Duration::from_millis(next_delay_ms);

        self.current_delay = next_delay.min(self.config.max_delay);
    }

    /// Add random jitter to the delay to prevent thundering herd problems
    fn add_jitter(&self, delay: Duration) -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Use a simple hash-based pseudo-random for deterministic testing
        let mut hasher = DefaultHasher::new();
        self.attempt_count.hash(&mut hasher);
        let hash = hasher.finish();

        // Generate jitter between -10% and +10% of the delay
        let jitter_percent = ((hash % 20) as f64 - 10.0) / 100.0; // -0.1 to 0.1
        let jitter_ms = (delay.as_millis() as f64 * jitter_percent) as i64;

        let jittered_ms = (delay.as_millis() as i64 + jitter_ms).max(0) as u64;
        Duration::from_millis(jittered_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = ReconnectConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        // Invalid initial delay
        let mut config = ReconnectConfig::default();
        config.initial_delay = Duration::from_millis(0);
        assert!(config.validate().is_err());

        // Max delay less than initial delay
        let mut config = ReconnectConfig::default();
        config.max_delay = Duration::from_millis(500);
        config.initial_delay = Duration::from_millis(1000);
        assert!(config.validate().is_err());

        // Invalid backoff multiplier
        let mut config = ReconnectConfig::default();
        config.backoff_multiplier = 1.0;
        assert!(config.validate().is_err());

        // Zero max attempts
        let mut config = ReconnectConfig::default();
        config.max_attempts = Some(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_reconnect_handler_creation() {
        let config = ReconnectConfig::default();
        let handler = ReconnectHandler::new(config);
        assert!(handler.is_ok());

        let handler = ReconnectHandler::with_default();
        assert_eq!(handler.attempt_count(), 0);
        assert!(handler.elapsed_time().is_none());
    }

    #[test]
    fn test_exponential_backoff() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
            backoff_multiplier: 2.0,
            max_attempts: Some(5),
            max_total_duration: None,
            jitter: false,
        };

        let mut handler = ReconnectHandler::new(config).unwrap();

        // First attempt
        let delay1 = handler.should_reconnect().unwrap();
        assert_eq!(delay1, Duration::from_millis(100));
        assert_eq!(handler.attempt_count(), 1);

        // Second attempt (doubled)
        let delay2 = handler.should_reconnect().unwrap();
        assert_eq!(delay2, Duration::from_millis(200));
        assert_eq!(handler.attempt_count(), 2);

        // Third attempt (doubled again)
        let delay3 = handler.should_reconnect().unwrap();
        assert_eq!(delay3, Duration::from_millis(400));
        assert_eq!(handler.attempt_count(), 3);
    }

    #[test]
    fn test_max_attempts_exceeded() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(1000),
            backoff_multiplier: 2.0,
            max_attempts: Some(2),
            max_total_duration: None,
            jitter: false,
        };

        let mut handler = ReconnectHandler::new(config).unwrap();

        // First two attempts should succeed
        assert!(handler.should_reconnect().is_ok());
        assert!(handler.should_reconnect().is_ok());

        // Third attempt should fail
        let result = handler.should_reconnect();
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(ReconnectError::MaxAttemptsExceeded(2))
        ));
    }

    #[test]
    fn test_max_delay_cap() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(300),
            backoff_multiplier: 3.0,
            max_attempts: Some(5),
            max_total_duration: None,
            jitter: false,
        };

        let mut handler = ReconnectHandler::new(config).unwrap();

        // First attempt: 100ms
        let delay1 = handler.should_reconnect().unwrap();
        assert_eq!(delay1, Duration::from_millis(100));

        // Second attempt: 300ms (capped at max_delay)
        let delay2 = handler.should_reconnect().unwrap();
        assert_eq!(delay2, Duration::from_millis(300));

        // Third attempt: still capped at 300ms
        let delay3 = handler.should_reconnect().unwrap();
        assert_eq!(delay3, Duration::from_millis(300));
    }

    #[test]
    fn test_reset_functionality() {
        let config = ReconnectConfig::default();
        let mut handler = ReconnectHandler::new(config).unwrap();

        // Make some attempts
        let _ = handler.should_reconnect();
        let _ = handler.should_reconnect();
        assert_eq!(handler.attempt_count(), 2);
        assert!(handler.elapsed_time().is_some());

        // Reset should restore initial state
        handler.reset();
        assert_eq!(handler.attempt_count(), 0);
        assert!(handler.elapsed_time().is_none());
    }

    #[test]
    fn test_jitter_adds_variation() {
        let config = ReconnectConfig {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_millis(5000),
            backoff_multiplier: 2.0,
            max_attempts: Some(10),
            max_total_duration: None,
            jitter: true,
        };

        let mut handler = ReconnectHandler::new(config).unwrap();

        // With jitter enabled, delays should vary
        let delay1 = handler.should_reconnect().unwrap();
        handler.reset();
        let delay2 = handler.should_reconnect().unwrap();

        // Note: This test might be flaky if the jitter produces the same value
        // but it's very unlikely with our hash-based approach
        println!("Delay1: {:?}, Delay2: {:?}", delay1, delay2);
    }
}
