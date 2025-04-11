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
