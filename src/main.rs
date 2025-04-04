use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use hyper::body::{Body, HttpBody};
use hyper::client::HttpConnector;
use hyper::header::{HeaderName, HeaderValue};
use hyper::http::uri::Scheme;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Client, Method, Request, Response, Server, StatusCode, Uri};
use hyper_util::rt::TokioExecutor;
use log::{error, info, warn};
use reqwest::{Body, Request, Response};
use tokio::sync::Mutex;
use tokio::time::sleep;

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

    fn all_backends(&self) -> Vec<String> {
        self.backends.clone()
    }
}

fn clone_headers(src_req: &Request<Body>, dst_req: &mut Request<Body>) {
    for (name, value) in src_req.headers() {
        if name != hyper::header::HOST {
            dst_req.headers_mut().insert(name.clone(), value.clone());
        }
    }
}

async fn forward_request(
    client: &Client,
    backend: &str,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    let uri_string = format!(
        "{}{}",
        backend,
        req.uri().path_and_query().map_or("", |p| p.as_str())
    );
    let uri: Uri = uri_string.parse().unwrap();

    let mut new_req = Request::builder()
        .method(req.method())
        .uri(uri)
        .body(req.into_body())
        .unwrap();

    clone_headers(&req, &mut new_req);

    client.request(new_req).await
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
