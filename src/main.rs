use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::u32;

use bytes::Bytes;
use futures::StreamExt;
use hyper::body::{Body, HttpBody};
use hyper::client::{self, HttpConnector};
use hyper::header::{HeaderName, HeaderValue};
use hyper::http::uri::Scheme;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Client, Method, Request, Response, Server, StatusCode, Uri};
use hyper_util::rt::TokioExecutor;
use log::{error, info, warn};
use tokio::sync::Mutex;
use tokio::time::sleep;

enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
}

#[derive(Debug, Clone, PartialEq)]
enum HealthStatus {
    Healthy,
    Unhealthy(u32),
}

struct Backend {
    url: String,
    health_status: HealthStatus,
    weight: u32,
    current_weight: i32,
}

struct LoadBalancer {
    backends: Vec<Backend>,
    current_idx: usize,
    max_failures: u32,
    strategy: LoadBalancingStrategy,
}

impl LoadBalancer {
    fn new(backend_urls: Vec<String>, max_failures: u32) -> Self {
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
            strategy: LoadBalancingStrategy::RoundRobin,
        }
    }

    fn new_weighted(backend_with_weights: Vec<(String, u32)>, max_failures: u32) -> Self {
        let mut backends = Vec::new();

        for (url, weight) in backend_with_weights {
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
            strategy: LoadBalancingStrategy::WeightedRoundRobin,
        }
    }

    fn get_next_backends(&mut self) -> Option<String> {
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
    client: &Client<HttpConnector>,
    backend: &str,
    req: hyper::Request<Body>,
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

async fn handle_request(
    req: Request<Body>,
    lb: Arc<Mutex<LoadBalancer>>,
    client: Client<HttpConnector>,
) -> Result<Response<Body>, Infallible> {
    info!("Received request: {} {}", req.method(), req.uri());

    let backend = {
        let mut lb = lb.lock().await;
        lb.get_next_backends()
    };

    match backend {
        Some(backend_url) => {
            info!("Forwarding request to backend: {}", backend_url);

            match forward_request(&client, &backend_url, req).await {
                Ok(response) => {
                    info!(
                        "Received response from backend {} with status {}",
                        backend_url,
                        response.status()
                    );

                    let mut lb = lb.lock().await;
                    lb.mark_healthy(&backend_url);

                    Ok(response)
                }
                Err(e) => {
                    error!("Error forwarding request to {}: {}", backend_url, e);

                    let mut lb = lb.lock().await;
                    lb.mark_unhealthy(&backend_url);

                    let response = Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .body(Body::from("Service Unavailable"))
                        .unwrap();

                    Ok(response)
                }
            }
        }
        None => {
            error!("No healthy backends available");

            let response = Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Body::from("No healthy backends available"))
                .unwrap();

            Ok(response)
        }
    }
}

async fn health_check(lb: Arc<Mutex<LoadBalancer>>, client: Client<HttpConnector>) {
    let interval = Duration::from_secs(10);

    loop {
        sleep(interval).await;

        let backends = {
            let lb = lb.lock().await;
            lb.get_next_backends()
        };

        for backend in backends {
            info!("Performing health check on {}", backend);

            let url = format!("{}/health", backend);
            let req = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .unwrap();

            match client.request(req).await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Health check succeeded for {}", backend);
                        let mut lb = lb.lock().await;
                        lb.mark_healthy(&backend);
                    } else {
                        warn!(
                            "Health check failed for {} with status {}",
                            backend,
                            response.status()
                        );

                        let mut lb = lb.lock().await;
                        lb.mark_unhealthy(&backend);
                    }
                }

                Err(e) => {
                    error!("Health check error {} : {}", backend, e);
                    let mut lb = lb.lock().await;
                    lb.mark_unhealthy(&backend);
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

    let load_balancer = Arc::new(Mutex::new(LoadBalancer::new(backends, 3)));

    let client = Client::new();

    let lb_health = load_balancer.clone();
    let client_health = client.clone();

    tokio::spawn(async move {
        health_check(lb_health, client_health).await;
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    info!("Starting load balancer on {}", addr);

    let lb_ref = load_balancer.clone();
    let make_service = make_service_fn(move |_| {
        let lb_clone = lb_ref.clone();
        let client_clone = client.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, lb_clone.clone(), client_clone.clone())
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        error!("Server error: {}", e);
    }

    info!("Load balancer stopped");
}
