use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use tokio::time::sleep;

async fn handle_request(
    req: Request<Body>,
    server_id: u16,
    delay: Option<Duration>,
) -> Result<Response<Body>, Infallible> {
    println!(
        "Server {} received request: {} {}",
        server_id,
        req.method(),
        req.uri()
    );

    if let Some(delay_duration) = delay {
        sleep(delay_duration).await;
    }

    if req.uri().path() == "/health" {
        return Ok(Response::new(Body::from("OK")));
    }

    let response_text = format!(
        "Hello from backend server {}!\nPath: {}\nMethod: {}\n",
        server_id,
        req.uri().path(),
        req.method()
    );

    Ok(Response::new(Body::from(response_text)))
}
