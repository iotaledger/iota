# Simulacrum Server

A gRPC and REST API server for IOTA Simulacrum that allows external clients to interact with a simulated IOTA blockchain.

## Features

- **REST API**: Full control interface for managing the simulacrum
- **gRPC API**: High-performance streaming interface
- **Command-line Interface**: Configurable server settings

## Quick Start

### Build and Run

```bash
# Run with default settings
cargo run --bin simulacrum-server

# Run with custom configuration
cargo run --bin simulacrum-server -- \
    --rest-address 127.0.0.1:8080 \
    --grpc-address 127.0.0.1:9000 \
    --initial-checkpoints 5
```

### Command Line Options

- `--grpc-address`: gRPC server address (default: 127.0.0.1:9000)
- `--rest-address`: REST API server address (default: 127.0.0.1:8080)
- `--initial-checkpoints`: Number of checkpoints to create on startup (default: 0)
- `--chain-start-timestamp-ms`: Chain start timestamp in milliseconds

## REST API Endpoints

### Status

- `GET /status` - Detailed simulacrum status

### Checkpoint Management

- `GET /checkpoint` - Get the latest checkpoint
- `POST /checkpoint/create` - Create a new checkpoint
- `POST /checkpoint/create_multiple` - Create multiple checkpoints

### Time and Epoch Control

- `POST /clock/advance` - Advance the simulacrum clock
- `POST /epoch/advance` - Advance to the next epoch

## API Examples

Run the included demo script to see all endpoints in action:

```bash
./examples/rest_api_demo.sh
```
