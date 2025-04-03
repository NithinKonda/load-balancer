use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::usize;
use tokio::sync::Mutex;

#[derive()]
enum HealthStatus {
    Healthy,
    Unhealthy(u32),
}

struct LoadBalancer {
    backends: Vec<String>,
    current_idx: usize,
    health_status: Vec<HealthStatus>,
    max_failures: u32,
}

impl LoadBalancer {
    fn new(backends: Vec<String>, max_failures: u32) -> Self {
        let health_status = vec![HealthStatus::Healthy; backends.len()];
        LoadBalancer {
            backends,
            current_idx: 0,
            health_status,
            max_failures,
        }
    }

    fn get_next_backend(&mut self) -> Option<String> {
        if self.backends.is_empty() {
            return None;
        }

        let backend = self.backends[self.current_idx].clone();
        self.current_idx = (self.current_idx + 1) % self.backends.len();

        Some(backend)
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let backends = vec![
        "http://localhost:9001".to_string(),
        "http://localhost:9002".to_string(),
        "http://localhost:9003".to_string(),
    ];

    let load_balancer = Arc::new(Mutex::new(LoadBalancer::new(backends)));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    info!("Starting load balancer on {}", addr);

    info!("Load Balancer Stopped");
}
