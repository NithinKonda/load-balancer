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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} SERVER_ID [DELAY_MS]", args[0]);
        eprintln!("  SERVER_ID: The ID of this server (1, 2, 3, etc.)");
        eprintln!("  DELAY_MS: Optional artificial delay in milliseconds");
        return Ok(());
    }

    let server_id = args[1].parse::<u16>().map_err(|_| "Invalid server ID")?;

    let delay = if args.len() > 2 {
        let delay_ms = args[2].parse::<u64>().map_err(|_| "Invalid delay")?;
        Some(Duration::from_millis(delay_ms))
    } else {
        None
    };

    let port = 9000 + server_id;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    println!("Starting backend server {} on {}...", server_id, addr);
    if let Some(delay_duration) = delay {
        println!("  With artificial delay: {:?}", delay_duration);
    }

    let server_id_clone = server_id;
    let delay_clone = delay;
    let make_service = make_service_fn(move |_| {
        let server_id = server_id_clone;
        let delay = delay_clone;

        async move { Ok::<_, Infallible>(service_fn(move |req| handle_request(req, server_id, delay))) }
    });

    let server = Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        eprintln!("Server error: {}", e);
    }

    Ok(())
}
