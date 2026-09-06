//! Provider health tracking (§21.4).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum HealthState {
    Healthy,
    /// Rate limited until the retry-after instant.
    RateLimited { retry_after_ms: u64 },
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct ProviderHealth {
    state: HealthState,
    updated: Instant,
}

impl ProviderHealth {
    pub fn healthy() -> Self {
        Self { state: HealthState::Healthy, updated: Instant::now() }
    }

    pub fn observe_rate_limited(&mut self, retry_after: Duration) {
        self.state = HealthState::RateLimited { retry_after_ms: retry_after.as_millis() as u64 };
        self.updated = Instant::now();
    }

    pub fn observe_unavailable(&mut self) {
        self.state = HealthState::Unavailable;
        self.updated = Instant::now();
    }

    pub fn observe_success(&mut self) {
        self.state = HealthState::Healthy;
        self.updated = Instant::now();
    }

    /// Current view: rate limits expire with wall time.
    pub fn state(&self) -> &HealthState {
        &self.state
    }

    pub fn is_available(&self, now: Instant) -> bool {
        match &self.state {
            HealthState::Healthy => true,
            HealthState::RateLimited { retry_after_ms } => {
                now.duration_since(self.updated).as_millis() as u64 >= *retry_after_ms
            }
            HealthState::Unavailable => false,
        }
    }
}

/// Per-route health registry.
#[derive(Default)]
pub struct Health {
    routes: parking_lot::Mutex<BTreeMap<String, ProviderHealth>>,
}

impl Health {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self, route: &str) -> ProviderHealth {
        self.routes
            .lock()
            .get(route)
            .cloned()
            .unwrap_or_else(ProviderHealth::healthy)
    }

    pub fn observe_success(&self, route: &str) {
        self.routes.lock().entry(route.to_owned()).or_insert_with(ProviderHealth::healthy).observe_success();
    }

    pub fn observe_rate_limited(&self, route: &str, retry_after: Duration) {
        self.routes
            .lock()
            .entry(route.to_owned())
            .or_insert_with(ProviderHealth::healthy)
            .observe_rate_limited(retry_after);
    }

    pub fn observe_unavailable(&self, route: &str) {
        self.routes.lock().entry(route.to_owned()).or_insert_with(ProviderHealth::healthy).observe_unavailable();
    }

    /// Availability check against a reference instant (deterministic tests).
    pub fn is_available(&self, route: &str, now: Instant) -> bool {
        match self.routes.lock().get(route) {
            Some(health) => health.is_available(now),
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_expires_with_time() {
        let health = Health::new();
        health.observe_rate_limited("prov/m", Duration::from_millis(50));
        let start = Instant::now();
        assert!(!health.is_available("prov/m", start));
        assert!(health.is_available("prov/m", start + Duration::from_millis(60)));
    }

    #[test]
    fn success_clears_failures() {
        let health = Health::new();
        health.observe_unavailable("prov/m");
        assert!(!health.is_available("prov/m", Instant::now()));
        health.observe_success("prov/m");
        assert!(health.is_available("prov/m", Instant::now()));
    }

    #[test]
    fn unknown_routes_default_healthy() {
        let health = Health::new();
        assert!(health.is_available("never-seen/m", Instant::now()));
    }
}
