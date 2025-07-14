# ledger-transport-tcp

A TCP transport implementation for communicating with Speculos, the Ledger hardware wallet simulator.

## Overview

This crate provides a TCP-based transport layer for communicating with [Speculos](https://github.com/LedgerHQ/speculos), the official Ledger hardware wallet simulator. It implements the APDU (Application Protocol Data Unit) exchange protocol using TCP sockets, enabling developers to test Ledger app interactions without requiring physical hardware.

## Features

- TCP-based transport for Speculos simulator communication
- APDU command/response handling
- Big-endian length-prefixed message protocol
- Error handling for connection and communication issues
- Development and testing without physical Ledger hardware

## Usage

```rust
use ledger_transport_tcp::TransportTCP;
use ledger_transport::APDUCommand;

// Create a new TCP transport instance to connect to Speculos
// Default Speculos APDU port is 9999
let transport = TransportTCP::new("localhost", 9999);

// Create an APDU command
let command = APDUCommand {
    cla: 0x80,
    ins: 0x02,
    p1: 0x00,
    p2: 0x00,
    data: vec![],
};

// Exchange the command with the Speculos simulator
match transport.exchange(&command) {
    Ok(response) => {
        println!("Response: {:?}", response);
    },
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Protocol

The TCP transport uses a simple length-prefixed protocol:

1. Send 4 bytes (big-endian) indicating the length of the APDU command
2. Send the APDU command bytes
3. Receive 4 bytes (big-endian) indicating the length of the response
4. Receive the APDU response bytes (including 2-byte status code)

## Error Handling

The crate defines three main error types:

- `ConnectError`: Failed to establish TCP connection
- `ResponseError`: Error parsing APDU response
- `InnerError`: I/O error during communication