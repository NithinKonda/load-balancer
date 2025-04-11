#!/bin/bash

# Compile the backend server
echo "Compiling backend server..."
rustc backend_server.rs -o backend_server

# Start three backend servers with different delays
echo "Starting backend servers..."
./backend_server 1 0 &   # Server 1 - No delay
./backend_server 2 100 & # Server 2 - 100ms delay
./backend_server 3 200 & # Server 3 - 200ms delay

echo "Backend servers are running. Press Ctrl+C to stop all."
wait
