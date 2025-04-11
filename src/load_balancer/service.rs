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
