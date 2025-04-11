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
}
