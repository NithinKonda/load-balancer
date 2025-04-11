# Rust HTTP Load Balancer

A custom HTTP load balancer built in Rust using Tokio and Hyper. This load balancer distributes incoming HTTP requests across multiple backend servers using various load balancing strategies.

## Features

- **Multiple Load Balancing Strategies**:
  - Round Robin
  - Weighted Round Robin
  - Sticky Sessions (based on client IP)
  
- **Health Checking**:
  - Periodic health checks of backend servers
  - Automatic removal of unhealthy backends
  - Automatic re-addition of recovered backends
  
- **Configuration**:
  - JSON-based configuration file
  - Dynamic configuration via HTTP endpoints
  
- **Error Handling**:
  - Graceful handling of backend failures
  - Configurable retry policies
  - 503 Service Unavailable responses when all backends are down

## Getting Started

### Prerequisites

- Rust 1.50 or later
- Cargo

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/NithinKonda/load-balancer.git
   cd rust-load-balancer
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

### Configuration

The load balancer is configured using a JSON file (`config.json`). Here's an example configuration:

```json
{
  "listen_address": "127.0.0.1:8080",
  "strategy": "weighted",
  "backends": [
    {
      "url": "http://localhost:9001",
      "weight": 5
    },
    {
      "url": "http://localhost:9002",
      "weight": 3
    },
    {
      "url": "http://localhost:9003",
      "weight": 2
    }
  ],
  "health_check": {
    "path": "/health",
    "interval_seconds": 10,
    "timeout_seconds": 5,
    "max_failures": 3
  },
  "session": {
    "timeout_seconds": 300,
    "cookie_name": "lb_session"
  }
}
```

### Running the Load Balancer

```bash
cargo run --release
```

Or directly run the binary:

```bash
./target/release/rust-load-balancer
```

### Testing with Sample Backend Servers

The repository includes a simple backend server for testing. To use it:

1. Compile the backend server:
   ```bash
   rustc backend_server.rs -o backend_server
   ```

2. Run multiple instances with different ports and response delays:
   ```bash
   ./backend_server 1 0 &     # Server 1 - No delay
   ./backend_server 2 100 &   # Server 2 - 100ms delay
   ./backend_server 3 200 &   # Server 3 - 200ms delay
   ```

3. Use the provided test script to see how the load balancer works:
   ```bash
   ./test_load_balancer.sh
   ```

## Dynamic Configuration

You can modify the load balancer's behavior at runtime using these HTTP endpoints:

### Change Load Balancing Strategy

```
GET /admin/strategy?type=roundrobin
GET /admin/strategy?type=weighted
GET /admin/strategy?type=sticky
```

### Set Backend Weight

```
GET /admin/weight?backend=localhost:9001&weight=10
```

### Set Session Timeout

```
GET /admin/session-timeout?seconds=600
```

## Implementation Details

### Project Structure

- `src/main.rs` - Entry point and server initialization
- `src/config.rs` - Configuration parsing and validation
- `src/load_balancer.rs` - Core load balancing logic
- `src/load_balancer/service.rs` - HTTP request handling and forwarding
- `src/health_check.rs` - Backend health checking

### Core Components

1. **LoadBalancer**: Implements different load balancing algorithms and manages backend state.
2. **HealthCheck**: Periodically checks backend health and updates their status.
3. **RequestHandler**: Receives client requests, selects a backend, and forwards the request.

## Load Balancing Strategies

### Round Robin

The simplest strategy, where requests are distributed evenly across all healthy backends in sequence.

### Weighted Round Robin

Backends are assigned weights that determine how many requests they receive relative to other backends. A backend with weight 5 will receive approximately 5 times as many requests as a backend with weight 1.

### Sticky Sessions

Requests from the same client IP address are consistently routed to the same backend server, as long as that backend remains healthy.

## Performance Considerations

- Uses Tokio for asynchronous I/O
- Hyper for high-performance HTTP handling
- DashMap for concurrent access to shared state

