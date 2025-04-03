use log::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::usize;
use tokio::sync::Mutex;

struct LoadBalancer {
    backends: Vec<String>,
    current_idx: usize,
}

impl LoadBalancer {
    fn new(backends: Vec<String>) -> Self {
        LoadBalancer {
            backends,
            current_idx: 0,
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
}
