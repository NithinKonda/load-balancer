use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::usize;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
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

        let start_idx = self.current_idx;
        loop {
            if let HealthStatus::Healthy = self.health_status[self.current_idx] {
                let backend = self.backends[self.current_idx].clone();
                self.current_idx = (self.current_idx + 1) % self.backends.len();

                return Some(backend);
            }

            self.current_idx = (self.current_idx + 1) % self.backends.len();

            if self.current_idx == start_idx {
                return None;
            }
        }
    }

    fn mark_unhealthy(&mut self, backend_url: &str) {
        if let Some(idx) = self.backends.iter().position(|url| url == backend_url) {
            match &self.health_status[idx] {
                HealthStatus::Healthy => {
                    self.health_status[idx] = HealthStatus::Unhealthy(1);
                    warn!("Backend {} marked as Unhealthy (1 failure)", backend_url);
                }

                HealthStatus::Unhealthy(failures) => {
                    let new_failures = failures + 1;
                    self.health_status[idx] = HealthStatus::Unhealthy(new_failures);
                    warn!(
                        "Backend {} remains unhealthy ({} failures)",
                        backend_url, new_failures
                    );
                }
            }
        }
    }

    fn mark_healthy(&mut self, backend_url: &str) {
        if let Some(idx) = self.backends.iter().position(|url| url == backend_url) {
            match &self.health_status[idx] {
                HealthStatus::Healthy => {}

                HealthStatus::Unhealthy(_) => {
                    self.health_status[idx] = HealthStatus::Healthy;
                    info!("Backend {} marked as healthy", backend_url);
                }
            }
        }
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
