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
}
