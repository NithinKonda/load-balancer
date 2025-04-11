pub mod service;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use log::{info, warn};

use crate::config::{LoadBalancerConfig, Strategy};

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(u32),
}

pub struct Backend {
    pub url: String,
    pub health_status: HealthStatus,
    pub weight: u32,
    pub current_weight: i32,
}

pub struct SessionInfo {
    pub backend_url: String,
    pub last_seen: Instant,
}

pub struct LoadBalancer {
    // List of backend servers with metadata
    pub backends: Vec<Backend>,
    // Current index for simple round-robin selection
    current_idx: usize,
    // Maximum failures before considering a backend unhealthy
    max_failures: u32,
    // Current load balancing strategy
    strategy: Strategy,
    // Session sticky mapping (client IP -> backend)
    sessions: HashMap<String, SessionInfo>,
    // Session timeout in seconds
    session_timeout: u64,
    // Configuration
    config: LoadBalancerConfig,
}

impl LoadBalancer {
    pub fn new(backend_urls: Vec<String>, max_failures: u32, config: LoadBalancerConfig) -> Self {
        let mut backends = Vec::new();

        for url in backend_urls {
            backends.push(Backend {
                url,
                health_status: HealthStatus::Healthy,
                weight: 1,
                current_weight: 0,
            });
        }

        LoadBalancer {
            backends,
            current_idx: 0,
            max_failures,
            strategy: Strategy::RoundRobin,
            sessions: HashMap::new(),
            session_timeout: config.session.timeout_seconds,
            config,
        }
    }

    pub fn new_weighted(
        backends_with_weights: Vec<(String, u32)>,
        max_failures: u32,
        config: LoadBalancerConfig,
    ) -> Self {
        let mut backends = Vec::new();

        for (url, weight) in backends_with_weights {
            backends.push(Backend {
                url,
                health_status: HealthStatus::Healthy,
                weight,
                current_weight: 0,
            });
        }

        LoadBalancer {
            backends,
            current_idx: 0,
            max_failures,
            strategy: Strategy::WeightedRoundRobin,
            sessions: HashMap::new(),
            session_timeout: config.session.timeout_seconds,
            config,
        }
    }

    pub fn set_strategy(&mut self, strategy: Strategy) {
        self.strategy = strategy;
    }

    pub fn set_weight(&mut self, backend_url: &str, weight: u32) {
        if let Some(backend) = self.backends.iter_mut().find(|b| b.url == backend_url) {
            backend.weight = weight;
            info!("Set weight {} for backend {}", weight, backend_url);
        } else {
            warn!("Backend {} not found when setting weight", backend_url);
        }
    }

    pub fn set_session_timeout(&mut self, timeout: u64) {
        self.session_timeout = timeout;
        info!("Set session timeout to {} seconds", timeout);
    }

    fn cleanup_expired_sessions(&mut self) {
        self.sessions = self
            .sessions
            .drain()
            .filter(|(_, session)| {
                now.duration_since(session.last_seen).as_secs() < self.session_timeout
            })
            .collect();
    }

    fn get_next_backend_round_robin(&mut self) -> Option<String> {
        if self.backends.is_empty() {
            return None;
        }

        let start_idx = self.current_idx;
        loop {
            if let HealthStatus::Healthy = self.backends[self.current_idx].health_status {
                let backend = self.backends[self.current_idx].url.clone();
                self.current_idx = (self.current_idx + 1) % self.backends.len();
                return Some(backend);
            }

            self.current_idx = (self.current_idx + 1) % self.backends.len();

            if self.current_idx == start_idx {
                return None;
            }
        }
    }
}
