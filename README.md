# CTMP Proxy Server

A TCP proxy server implementation for the CoreTech Message Protocol (CTMP) written in Rust. Submitted as part of the Operation Wirestorm competition.

## Overview

This server acts as a message forwarding proxy that:
- Accepts a single source client on port `33333`
- Accepts multiple destination clients on port `44444` 
- Forwards CTMP-compliant messages from source to all connected destinations
- Validates message headers and handles client disconnections gracefully

## CoreTech Message Protocol (CTMP)

The server implements the CTMP specification with the following message format:

```
    0               1               2               3
    0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | MAGIC 0xCC    | PADDING       | LENGTH                      |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | PADDING                                                     |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | DATA ...................................................... |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **MAGIC**: 8 bits, must be `0xCC`
- **PADDING**: 8 bits, must be `0x00`
- **LENGTH**: 16 bits, unsigned, network byte order, payload size (max 65,535 bytes)
- **PADDING**: 32 bits, must be `0x00000000`
- **DATA**: Variable length payload

## System Requirements

- **Ubuntu 24.04 LTS**
- **Rust 1.70+** (uses standard library only)
- **TCP ports 33333 and 44444** available

## Installation

### 1. Install Rust (if not already installed)

```bash
# Install Rust via rustup 
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload shell environment
source ~/.bashrc

# Verify installation
rustc --version
cargo --version
```

### 2. Clone and Build

```bash
# Clone the repository
git clone https://github.com/jarrowsmith123/operation-wirestorm.git
cd operation-wirestorm

# Build the project
cargo build
```

## Usage

### Start the Server

```bash

# Run directly with cargo
cargo run

# Or run the compiled binary
./target/release/wirestorm


```

Expected output:
```
Starting CTMP Proxy Server...
Listening for destination clients on 127.0.0.1:44444
Listening for single source client on 127.0.0.1:33333
```


## Testing with Provided Python Tests

The project includes the supplied Python 3.12 tests for the competition.

### Prerequisites for Testing

```bash
# Ensure Python 3.12+ is installed
python3 --version
```

### Running the Tests

```bash
# 1. Start the server in one terminal
cargo run

# 2. In another terminal, run the provided tests
python3 tests.py
```


## Configuration

The server uses these default constants (modify in `src/main.rs` if needed):

```rust
const SOURCE_ADDR: &str = "127.0.0.1:33333";      // Source client port
const DESTINATION_ADDR: &str = "127.0.0.1:44444"; // Destination clients port
const MAX_DESTINATIONS: usize = 100;              // Maximum destination clients
const READ_TIMEOUT_SECS: u64 = 10;                // Client read timeout
```

## Architecture

- Main thread handles one source client at a time while background thread accepts destination clients
- Uses `Arc<Mutex<>>` for safe concurrent access to destination list
- Automatic cleanup of disconnected clients
- Strictly defined CTMP header struct for readability and validation

## Development

### Project Structure
```
ctmp-proxy/
├── Cargo.toml          # Project configuration
├── src/
│   └── main.rs         # Main server implementation
├── tests.py            # Python test suite (provided)
├── client.py           # Part of test suite utilities
├── buffers.py          # Part of test suite utilities
├── README.md           # This file
├── LICENSE             # Project license
└── target/             # Build artifacts
    └── release/
        └── ctmp-proxy  # Compiled binary
```

## Security Considerations

- Maximum destination clients to prevent server being overwhelmed
- Read timeouts prevent slow client attacks 
- All CTMP headers are validated before processing

## Troubleshooting

### Port Already in Use
```bash
# Check what's using the ports
sudo netstat -tlnp | grep -E ':(33333|44444)'

# Kill processes using the ports if needed
sudo fuser -k 33333/tcp
sudo fuser -k 44444/tcp
```

### Firewall Issues
```bash
# Allow ports through UFW (if enabled)
sudo ufw allow 33333/tcp
sudo ufw allow 44444/tcp
```


## License

This project uses the Apache 2.0 license - http://www.apache.org/licenses/


