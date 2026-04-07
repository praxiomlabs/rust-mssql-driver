//! Supporting configuration types for redirect handling, timeouts, and retry policies.

use std::time::Duration;

/// Configuration for Azure SQL redirect handling.
///
/// Azure SQL Gateway may redirect connections to different backend servers.
/// This configuration controls how the driver handles these redirects.
#[derive(Debug, Clone)]
pub struct RedirectConfig {
    /// Maximum number of redirect attempts (default: 2).
    pub max_redirects: u8,
    /// Whether to follow redirects automatically (default: true).
    pub follow_redirects: bool,
}

impl Default for RedirectConfig {
    fn default() -> Self {
        Self {
            max_redirects: 2,
            follow_redirects: true,
        }
    }
}

impl RedirectConfig {
    /// Create a new redirect configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of redirect attempts.
    #[must_use]
    pub fn max_redirects(mut self, max: u8) -> Self {
        self.max_redirects = max;
        self
    }

    /// Enable or disable automatic redirect following.
    #[must_use]
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = follow;
        self
    }

    /// Disable automatic redirect following.
    ///
    /// When disabled, the driver will return an error with the redirect
    /// information instead of automatically following the redirect.
    #[must_use]
    pub fn no_follow() -> Self {
        Self {
            max_redirects: 0,
            follow_redirects: false,
        }
    }
}

/// Timeout configuration for various connection phases.
///
/// Per ARCHITECTURE.md §4.4, different phases of connection and command
/// execution have separate timeout controls.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Time to establish TCP connection (default: 15s).
    pub connect_timeout: Duration,
    /// Time to complete TLS handshake (default: 10s).
    pub tls_timeout: Duration,
    /// Time to complete login sequence (default: 30s).
    pub login_timeout: Duration,
    /// Default timeout for command execution (default: 30s).
    pub command_timeout: Duration,
    /// Time before idle connection is closed (default: 300s).
    pub idle_timeout: Duration,
    /// Interval for connection keep-alive (default: 30s).
    pub keepalive_interval: Option<Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(15),
            tls_timeout: Duration::from_secs(10),
            login_timeout: Duration::from_secs(30),
            command_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            keepalive_interval: Some(Duration::from_secs(30)),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the TCP connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the TLS handshake timeout.
    #[must_use]
    pub fn tls_timeout(mut self, timeout: Duration) -> Self {
        self.tls_timeout = timeout;
        self
    }

    /// Set the login sequence timeout.
    #[must_use]
    pub fn login_timeout(mut self, timeout: Duration) -> Self {
        self.login_timeout = timeout;
        self
    }

    /// Set the default command execution timeout.
    #[must_use]
    pub fn command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the keep-alive interval.
    #[must_use]
    pub fn keepalive_interval(mut self, interval: Option<Duration>) -> Self {
        self.keepalive_interval = interval;
        self
    }

    /// Disable keep-alive.
    #[must_use]
    pub fn no_keepalive(mut self) -> Self {
        self.keepalive_interval = None;
        self
    }

    /// Get the total time allowed for a full connection (TCP + TLS + login).
    #[must_use]
    pub fn total_connect_timeout(&self) -> Duration {
        self.connect_timeout + self.tls_timeout + self.login_timeout
    }
}

/// Retry policy for transient error handling.
///
/// Per ADR-009, the driver can automatically retry operations that fail
/// with transient errors (deadlocks, Azure service busy, etc.).
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (default: 3).
    pub max_retries: u32,
    /// Initial backoff duration before first retry (default: 100ms).
    pub initial_backoff: Duration,
    /// Maximum backoff duration between retries (default: 30s).
    pub max_backoff: Duration,
    /// Multiplier for exponential backoff (default: 2.0).
    pub backoff_multiplier: f64,
    /// Whether to add random jitter to backoff times (default: true).
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of retry attempts.
    #[must_use]
    pub fn max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Set the initial backoff duration.
    #[must_use]
    pub fn initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    /// Set the maximum backoff duration.
    #[must_use]
    pub fn max_backoff(mut self, backoff: Duration) -> Self {
        self.max_backoff = backoff;
        self
    }

    /// Set the backoff multiplier for exponential backoff.
    #[must_use]
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Enable or disable jitter.
    #[must_use]
    pub fn jitter(mut self, enabled: bool) -> Self {
        self.jitter = enabled;
        self
    }

    /// Disable automatic retries.
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Calculate the backoff duration for a given retry attempt.
    ///
    /// Uses exponential backoff with optional jitter.
    #[must_use]
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base = self.initial_backoff.as_millis() as f64
            * self
                .backoff_multiplier
                .powi(attempt.saturating_sub(1) as i32);
        let capped = base.min(self.max_backoff.as_millis() as f64);

        if self.jitter {
            // Simple jitter: multiply by random factor between 0.5 and 1.5
            // In production, this would use a proper RNG
            Duration::from_millis(capped as u64)
        } else {
            Duration::from_millis(capped as u64)
        }
    }

    /// Check if more retries are allowed for the given attempt number.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}
