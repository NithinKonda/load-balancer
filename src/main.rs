// STEP 4 DONE
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

    fn set_strategy(&mut self, strategy: LoadBalancingStrategy) {
        self.strategy = strategy;
    }

    fn set_weight(&mut self, backend_url: &str, weight: u32) {
        if let Some(backend) = self.backends.iter_mut().find(|b| b.url == backend_url) {
            backend.weight = weight;
            info!("Set weight {} for backend {}", weight, backend_url);
        } else {
            warn!("Backend {} not found when setting weight", backend_url);
        }
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

    fn get_next_backend_weighted(&mut self) -> Option<String> {
        let has_healthy = self
            .backends
            .iter()
            .any(|b| matches!(b.health_status, HealthStatus::Healthy));
        if !has_healthy {
            return None;
        }

        let mut total = 0;
        let mut best_idx = 0;
        let mut best_weight = -1;

        for (i, backend) in self.backends.iter_mut().enumerate() {
            if matches!(backend.health_status, HealthStatus::Healthy) {
                total += backend.weight as i32;
                backend.current_weight += backend.weight as i32;

                if backend.current_weight > best_weight {
                    best_weight = backend.current_weight;
                    best_idx = i;
                }
            }
        }

        if best_weight < 0 {
            return None;
        }

        self.backends[best_idx].current_weight -= total;

        Some(self.backends[best_idx].url.clone())
    }

    fn get_next_backend(&mut self) -> Option<String> {
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.get_next_backend_round_robin(),
            LoadBalancingStrategy::WeightedRoundRobin => self.get_next_backend_weighted(),
        }
    }

    fn mark_unhealthy(&mut self, backend_url: &str) {
        if let Some(backend) = self.backends.iter_mut().find(|b| b.url == backend_url) {
            match &backend.health_status {
                HealthStatus::Healthy => {
                    backend.health_status = HealthStatus::Unhealthy(1);
                    warn!("Backend {} marked as unhealthy (1 failure)", backend_url);
                }
                HealthStatus::Unhealthy(failures) => {
                    let new_failures = failures + 1;
                    backend.health_status = HealthStatus::Unhealthy(new_failures);
                    warn!(
                        "Backend {} remains unhealthy ({} failures)",
                        backend_url, new_failures
                    );
                }
            }
        }
    }

    fn mark_healthy(&mut self, backend_url: &str) {
        if let Some(backend) = self.backends.iter_mut().find(|b| b.url == backend_url) {
            match &backend.health_status {
                HealthStatus::Healthy => {}
                HealthStatus::Unhealthy(_) => {
                    backend.health_status = HealthStatus::Healthy;
                    info!("Backend {} marked as healthy", backend_url);
                }
            }
        }
    }

    fn get_all_backends(&self) -> Vec<String> {
        self.backends.iter().map(|b| b.url.clone()).collect()
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

async fn handle_request(
    req: Request<Body>,
    lb: Arc<Mutex<LoadBalancer>>,
    client: Client<HttpConnector>,
) -> Result<Response<Body>, Infallible> {
    info!("Received request: {} {}", req.method(), req.uri());

    if req.uri().path() == "/admin/strategy" {
        let query = req.uri().query().unwrap_or("");
        if query.contains("type=weighted") {
            let mut lb = lb.lock().await;
            lb.set_strategy(LoadBalancingStrategy::WeightedRoundRobin);
            info!("Changed load balancing strategy to Weighted Round Robin");
            return Ok(Response::new(Body::from(
                "Strategy changed to Weighted Round Robin",
            )));
        } else if query.contains("type=roundrobin") {
            let mut lb = lb.lock().await;
            lb.set_strategy(LoadBalancingStrategy::RoundRobin);
            info!("Changed load balancing strategy to Round Robin");
            return Ok(Response::new(Body::from("Strategy changed to Round Robin")));
        }
    }

    if req.uri().path() == "/admin/weight" {
        if let Some(query) = req.uri().query() {
            let params: Vec<&str> = query.split('&').collect();
            let mut backend = None;
            let mut weight = None;

            for param in params {
                let kv: Vec<&str> = param.split('=').collect();
                if kv.len() == 2 {
                    match kv[0] {
                        "backend" => backend = Some(kv[1]),
                        "weight" => weight = kv[1].parse::<u32>().ok(),
                        _ => {}
                    }
                }
            }

            if let (Some(backend), Some(weight)) = (backend, weight) {
                let mut lb = lb.lock().await;
                lb.set_weight(&format!("http://{}", backend), weight);
                return Ok(Response::new(Body::from(format!(
                    "Weight for {} set to {}",
                    backend, weight
                ))));
            }
        }
    }

    let backend = {
        let mut lb = lb.lock().await;
        lb.get_next_backend()
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
            lb.get_all_backends()
        };

        for backend in backends {
            info!("Performing health check on {}", backend);

            let uri = format!("{}/health", backend);
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
                    error!("Health check error for {}: {}", backend, e);
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

    let backends_with_weights = vec![
        ("http://localhost:9001".to_string(), 5), // Higher weight (more traffic)
        ("http://localhost:9002".to_string(), 3), // Medium weight
        ("http://localhost:9003".to_string(), 2), // Lower weight (less traffic)
    ];

    let load_balancer = Arc::new(Mutex::new(LoadBalancer::new_weighted(
        backends_with_weights,
        3,
    )));

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
