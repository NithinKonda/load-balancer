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

pub async fn health_check(
    lb: Arc<Mutex<LoadBalancer>>,
    client: Client<HttpConnector>,
    config: HealthCheckConfig,
) {
    let interval = Duration::from_secs(config.interval_seconds);
    let timeout = Duration::from_secs(config.timeout_seconds);

    loop {
        sleep(interval).await;

        let backends = {
            let lb = lb.lock().await;
            lb.get_all_backends()
        };

        for backend in backends {
            info!("Performing health check on {}", backend);

            let uri = format!("{}{}", backend, config.path);
            let req = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .unwrap();

            let response_future = client.request(req);

            // TODO: Implement proper timeout handling
            // For now, we just await the response without timeout
            match response_future.await {
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

pub fn start_health_checker(
    lb: Arc<Mutex<LoadBalancer>>,
    client: Client<HttpConnector>,
    config: HealthCheckConfig,
) {
    tokio::spawn(async move {
        health_check(lb, client, config).await;
    });
}
