#!/bin/bash

# Function to send requests to the load balancer
send_requests() {
  local num_requests=$1
  local url=$2
  
  echo "Sending $num_requests requests to $url..."
  for i in $(seq 1 $num_requests); do
    echo "Request $i:"
    curl -s "$url"
    echo -e "\n---------------------------"
    sleep 0.1
  done
}

# Function to test changing the load balancing strategy
change_strategy() {
  local strategy=$1
  echo "Changing load balancing strategy to $strategy..."
  curl -s "http://localhost:8080/admin/strategy?type=$strategy"
  echo -e "\n---------------------------"
}

# Function to set backend weight
set_weight() {
  local backend=$1
  local weight=$2
  echo "Setting weight for $backend to $weight..."
  curl -s "http://localhost:8080/admin/weight?backend=$backend&weight=$weight"
  echo -e "\n---------------------------"
}

# Main script

# Test round-robin strategy
echo "=== Testing Round Robin Strategy ==="
change_strategy "roundrobin"
send_requests 6 "http://localhost:8080/test"

# Test weighted round-robin strategy
echo "=== Testing Weighted Round Robin Strategy ==="
change_strategy "weighted"
send_requests 10 "http://localhost:8080/test"

# Change weights
echo "=== Changing Backend Weights ==="
set_weight "localhost:9001" 10
set_weight "localhost:9002" 2
set_weight "localhost:9003" 1
send_requests 10 "http://localhost:8080/test"

# Test sticky sessions
echo "=== Testing Sticky Sessions ==="
change_strategy "sticky"
send_requests 5 "http://localhost:8080/test"

echo "Test completed."
