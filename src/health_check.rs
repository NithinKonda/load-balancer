use std::sync::Arc;
use std::time::Duration;

use hyper::body::Body;
use hyper::client::HttpConnector;
use hyper::{Client, Method, Request};
use log::{error, info, warn};
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::config::HealthCheckConfig;
use crate::load_balancer::LoadBalancer;
