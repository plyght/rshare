# rshare

A Rust TUI application to securely expose localhost services to the internet with a custom tunnel implementation.

![rshare Screenshot](docs/screenshot.png)

## Features

- Beautiful terminal UI built with Ratatui
- Custom Rust-based tunneling implementation (no need for ngrok/cloudflared)
- No external dependencies for tunneling
- Custom domain support with full path handling (e.g., dev.peril.lol/api/users)
- Real-time connection logs
- TLS/SSL support via reverse proxy

## How It Works

rshare consists of two components:

1. **Client Mode**: Runs on your local machine and exposes a local port to the internet
2. **Server Mode**: Runs on a public server and acts as the tunnel endpoint

The tunnel works by:
- Establishing a WebSocket connection between the client and server
- Forwarding HTTP requests from the server to the client
- Routing responses back to the original requesters

## Quick Start

### Client Mode (default)

```bash
# Expose localhost:3000 with auto-assigned domain
cargo run -- --port 3000

# Expose localhost:3000 with custom domain
cargo run -- --port 3000 --domain myapp.dev.peril.lol

# Specify custom server port (if not using default 8000)
cargo run -- --port 3000 --public-port 9000
```

### Server Mode

You'll need to run this on a public server that can accept incoming connections.

```bash
# Run in server mode on default port 8000
cargo run -- --server

# Run in server mode on custom port
cargo run -- --server --public-port 9000
```

### Keyboard Shortcuts

- `s`: Start/stop tunnel
- `q`: Quit
- `↑/↓`: Scroll logs

## Building from source

```bash
cargo build --release
```

The compiled binary will be in `target/release/rshare`.

## Detailed Setup Guide

For complete instructions on setting up both client and server, including domain configuration, SSL certificates, and security best practices, see [SETUP.md](SETUP.md).

## Setting up Custom Domains

To use custom subdomains with dev.peril.lol, you'll need to:

1. Add a wildcard DNS record for *.dev.peril.lol pointing to your server
2. Configure SSL certificates for secure connections
3. Set up a reverse proxy for SSL termination (Nginx recommended)

Full details are provided in the [SETUP.md](SETUP.md) document.

## Security Considerations

- All traffic between the client and server is secured via WebSockets
- The server validates client requests to prevent unauthorized access
- For production, always use TLS (HTTPS) to encrypt all traffic
- Be cautious about which services you expose to the internet

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.