# CTMP Proxy Server

A TCP proxy server implementation for the CoreTech Message Protocol (CTMP) written in Rust. Submitted as part of the Operation Wirestorm - Reloaded competition.

## Overview

This server acts as a message forwarding proxy that:
- Accepts a single source client on port `33333`
- Accepts multiple destination clients on port `44444` 
- Forwards valid CTMP messages from source to all connected destinations
- Validates message headers and checksums for sensitive messages
- Handles client disconnections gracefully with automatic cleanup

## CoreTech Message Protocol (CTMP)

The server implements the extended CTMP specification with the following message format:

```
    0               1               2               3
    0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | MAGIC 0xCC    | OPTIONS       | LENGTH                      |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | CHECKSUM                      | PADDING                     |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    | DATA ...................................................... |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Header Fields

- **MAGIC**: 8 bits, must be `0xCC`
- **OPTIONS**: 8 bits, with bit 1 indicating sensitive messages (`0x40` for sensitive)
- **LENGTH**: 16 bits, unsigned, network byte order, payload size (max 65,535 bytes)
- **CHECKSUM**: 16 bits, unsigned, network byte order (used for sensitive message validation)
- **PADDING**: 16 bits, must be `0x0000`
- **DATA**: Variable length payload

### Options Field Layout

```
   0     1     2     3     4     5     6     7
+-----+-----+-----+-----+-----+-----+-----+-----+
|     |     |                                   |
| RES | SEN |              PADDING              |
|     |     |                                   |
+-----+-----+-----+-----+-----+-----+-----+-----+
```

- **Bit 0**: Reserved for Future Use
- **Bit 1**: 0 = Normal, 1 = Sensitive
- **Bits 2-7**: Padding

### Checksum Validation

For sensitive messages:
- The checksum field contains the 16-bit one's complement of the one's complement sum of all 16-bit words in the header and data
- During checksum calculation, the checksum field is treated as filled with `0xCC` bytes
- Invalid checksums cause the message to be dropped with an error logged

## System Requirements

- **Ubuntu 24.04 LTS**
- **Rust 1.70+**
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

### Message Processing Behavior

- **Normal Messages**: Forwarded immediately to all connected destinations
- **Sensitive Messages**: Checksum validated before forwarding
  - Valid checksums: Message forwarded normally
  - Invalid checksums: Message dropped, error logged, connection remains open
- **Invalid Headers**: Source client disconnected immediately
- **Connection Errors**: Affected clients disconnected and cleaned up automatically

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
const MAGIC: u8 = 0xCC;                           // CTMP magic byte
const HEADER_SIZE: usize = 8;                     // CTMP header size in bytes
const MAX_DESTINATIONS: usize = 100;              // Maximum destination clients
const READ_TIMEOUT_SECS: u64 = 10;                // Client read timeout
```

## Architecture

- Main thread handles one source client at a time while background thread accepts destination clients
- Uses `Arc<Mutex<>>` for safe concurrent access to destination list
- Automatic cleanup of disconnected clients
- Strictly defined CTMP header struct for readability and validation

1. Source client connects and sends CTMP messages
2. Server validates message headers (magic byte, padding)
3. For sensitive messages: checksum validation performed
4. Valid messages broadcasted to all connected destination clients -- Note that the length in the header is always trusted, data longer than that length in the stream is ignore - this is a potential drawback of the server --
5. Disconnected destination clients removed during broadcast


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
        └── wirestorm  # Compiled binary
```


## Security Considerations

- Maximum destination clients to prevent server being overwhelmed
- Read timeouts prevent slow client attacks 
- All CTMP headers are validated before processing
- Checksum validation to ensure corrupted messages not sent

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
