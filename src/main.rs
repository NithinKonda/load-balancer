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
}
fn main() {
    println!("Hello, world!");
}
