mod config;
mod health_check;
mod load_balancer;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Client, Server};
use log::{error, info};
use tokio::sync::Mutex;

use crate::config::{LoadBalancerConfig, Strategy};
use crate::health_check::start_health_checker;
use crate::load_balancer::LoadBalancer;
use crate::load_balancer::service::handle_request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config_path = "config.json";
    LoadBalancerConfig::generate_default(config_path)?;
    let config = LoadBalancerConfig::from_file(config_path)?;

    info!("Loaded configuration from {}", config_path);

    let load_balancer = match config.strategy {
        Strategy::RoundRobin => {
            let backend_urls: Vec<String> = config.backends.iter().map(|b| b.url.clone()).collect();

            Arc::new(Mutex::new(LoadBalancer::new(
                backend_urls,
                config.health_check.max_failures,
                config.clone(),
            )))
        }
        Strategy::WeightedRoundRobin | Strategy::StickySession => {
            let backends_with_weights: Vec<(String, u32)> = config
                .backends
                .iter()
                .map(|b| (b.url.clone(), b.weight.unwrap_or(1)))
                .collect();

            Arc::new(Mutex::new(LoadBalancer::new_weighted(
                backends_with_weights,
                config.health_check.max_failures,
                config.clone(),
            )))
        }
    };

    {
        let mut lb = load_balancer.lock().await;
        lb.set_strategy(config.strategy.clone());
        info!(
            "Initial load balancing strategy set to {:?}",
            config.strategy
        );
    }

    // Create an HTTP client for forwarding requests
    let client = Client::new();

    // Start the health checker
    start_health_checker(
        load_balancer.clone(),
        client.clone(),
        config.health_check.clone(),
    );

    // Parse the address to listen on
    let addr: SocketAddr = config.listen_address.parse()?;

    info!("Starting load balancer on {}", addr);

    // Create the service that will handle incoming requests
    let lb_ref = load_balancer.clone();
    let make_service = make_service_fn(move |conn| {
        let lb_clone = lb_ref.clone();
        let client_clone = client.clone();
        let remote_addr = conn.remote_addr();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle_request(req, lb_clone.clone(), client_clone.clone(), remote_addr)
            }))
        }
    });

    // Create and start the server
    let server = Server::bind(&addr).serve(make_service);

    // Run the server
    if let Err(e) = server.await {
        error!("Server error: {}", e);
    }

    info!("Load balancer stopped");

    Ok(())
}

