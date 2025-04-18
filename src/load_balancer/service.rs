use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use hyper::body::Body;
use hyper::client::HttpConnector;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Client, Request, Response, StatusCode, Uri};
use log::{error, info, warn};
use tokio::sync::Mutex;

use crate::config::Strategy;
use crate::load_balancer::LoadBalancer;

pub fn clone_headers(src_req: &Request<Body>, dst_req: &mut Request<Body>) {
    for (name, value) in src_req.headers() {
        if name != hyper::header::HOST {
            dst_req.headers_mut().insert(name.clone(), value.clone());
        }
    }
}

pub fn extract_client_ip(req: &Request<Body>) -> Option<String> {
    if let Some(forwarded_for) = req.headers().get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded_for.to_str() {
            let ips: Vec<&str> = forwarded_str.split(',').collect();
            if !ips.is_empty() {
                return Some(ips[0].trim().to_string());
            }
        }
    }

    if let Some(addr) = req.extensions().get::<SocketAddr>() {
        return Some(addr.ip().to_string());
    }

    None
}

pub async fn forward_request(
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

pub async fn handle_request(
    req: Request<Body>,
    lb: Arc<Mutex<LoadBalancer>>,
    client: Client<HttpConnector>,
    remote_addr: SocketAddr,
) -> Result<Response<Body>, Infallible> {
    info!(
        "Received request: {} {} from {}",
        req.method(),
        req.uri(),
        remote_addr
    );

    let mut req_with_addr = req;
    req_with_addr.extensions_mut().insert(remote_addr);

    let client_ip = extract_client_ip(&req_with_addr);

    if req_with_addr.uri().path() == "/admin/strategy" {
        let query = req_with_addr.uri().query().unwrap_or("");
        if query.contains("type=weighted") {
            let mut lb = lb.lock().await;
            lb.set_strategy(Strategy::WeightedRoundRobin);
            info!("Changed load balancing strategy to Weighted Round Robin");
            return Ok(Response::new(Body::from(
                "Strategy changed to Weighted Round Robin",
            )));
        } else if query.contains("type=roundrobin") {
            let mut lb = lb.lock().await;
            lb.set_strategy(Strategy::RoundRobin);
            info!("Changed load balancing strategy to Round Robin");
            return Ok(Response::new(Body::from("Strategy changed to Round Robin")));
        } else if query.contains("type=sticky") {
            let mut lb = lb.lock().await;
            lb.set_strategy(Strategy::StickySession);
            info!("Changed load balancing strategy to Sticky Session");
            return Ok(Response::new(Body::from(
                "Strategy changed to Sticky Session",
            )));
        }
    }

    if req_with_addr.uri().path() == "/admin/weight" {
        if let Some(query) = req_with_addr.uri().query() {
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
    if req_with_addr.uri().path() == "/admin/session-timeout" {
        if let Some(query) = req_with_addr.uri().query() {
            let params: Vec<&str> = query.split('&').collect();
            for param in params {
                let kv: Vec<&str> = param.split('=').collect();
                if kv.len() == 2 && kv[0] == "seconds" {
                    if let Ok(timeout) = kv[1].parse::<u64>() {
                        let mut lb = lb.lock().await;
                        lb.set_session_timeout(timeout);
                        return Ok(Response::new(Body::from(format!(
                            "Session timeout set to {} seconds",
                            timeout
                        ))));
                    }
                }
            }
        }
    }

    let backend = {
        let mut lb = lb.lock().await;
        lb.get_next_backend(client_ip.as_deref())
    };

    match backend {
        Some(backend_url) => {
            info!("Forwarding request to backend: {}", backend_url);

            match forward_request(&client, &backend_url, req_with_addr).await {
                Ok(mut response) => {
                    info!(
                        "Received response from backend {} with status {}",
                        backend_url,
                        response.status()
                    );

                    if {
                        let lb = lb.lock().await;
                        matches!(lb.strategy, Strategy::StickySession)
                    } {
                        let cookie_value = format!("backend={}; Path=/", backend_url);
                        response.headers_mut().insert(
                            hyper::header::SET_COOKIE,
                            HeaderValue::from_str(&cookie_value).unwrap(),
                        );
                    }

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
