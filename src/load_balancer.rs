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
