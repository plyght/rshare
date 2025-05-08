# rshare Setup Guide

This guide provides detailed instructions for setting up both the client and server components of rshare, as well as configuring your domain for proper tunneling.

## Server Setup

The server component must be deployed on a machine with a public IP address to allow incoming connections from the internet.

### Basic Requirements

- A VPS or cloud server with a public IP address (AWS, DigitalOcean, Linode, etc.)
- Ubuntu 20.04 or newer (other Linux distros should work too)
- Root or sudo access
- Open ports for both HTTP and WebSocket traffic

### Server Installation

1. Install Rust and dependencies:

```bash
# Update package lists
sudo apt update

# Install build essentials
sudo apt install -y build-essential pkg-config libssl-dev

# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

2. Clone and build the rshare repository:

```bash
git clone https://github.com/yourusername/rshare.git
cd rshare
cargo build --release
```

3. Running the server:

```bash
# Run on the default port 8000
./target/release/rshare --server

# Run on a custom port
./target/release/rshare --server --public-port 9000
```

### Firewall Configuration

You need to open the ports used by rshare on your server. The default ports are:
- 8000: WebSocket control channel
- 8001: HTTP tunnel service

```bash
# Using UFW (Ubuntu's default firewall)
sudo ufw allow 8000/tcp
sudo ufw allow 8001/tcp
sudo ufw reload

# Or using iptables
sudo iptables -A INPUT -p tcp --dport 8000 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 8001 -j ACCEPT
```

## Domain Configuration

To use custom domains with rshare, you'll need to configure DNS records.

### DNS Setup for dev.peril.lol

1. Log in to your domain registrar or DNS provider for peril.lol
2. Add the following DNS records:

```
# A record for the main domain
Type: A
Name: dev
Value: [Your server's public IP address]

# Wildcard CNAME record to allow subdomains
Type: CNAME
Name: *.dev
Value: dev.peril.lol
```

The wildcard CNAME is crucial as it allows any subdomain of dev.peril.lol to resolve to your server.

### SSL Certificate Setup

To enable HTTPS for your tunnels (required for most modern web apps), you'll need an SSL certificate:

1. Install Certbot for Let's Encrypt certificates:

```bash
sudo apt install -y certbot
```

2. Obtain a wildcard certificate:

```bash
sudo certbot certonly --manual --preferred-challenges dns \
  --server https://acme-v02.api.letsencrypt.org/directory \
  -d dev.peril.lol -d *.dev.peril.lol
```

Follow the on-screen instructions to add the required DNS TXT records.

3. Configure a reverse proxy (Nginx) for SSL termination:

```bash
sudo apt install -y nginx

# Create Nginx configuration
sudo nano /etc/nginx/sites-available/rshare
```

Add the following configuration:

```nginx
server {
    listen 80;
    server_name dev.peril.lol *.dev.peril.lol;
    
    # Redirect all HTTP to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    server_name dev.peril.lol *.dev.peril.lol;
    
    ssl_certificate /etc/letsencrypt/live/dev.peril.lol/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/dev.peril.lol/privkey.pem;
    
    # SSL settings (recommended)
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    
    # Proxy all requests to the rshare HTTP server
    location / {
        proxy_pass http://localhost:8001;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support (if needed)
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

Enable and restart Nginx:

```bash
sudo ln -s /etc/nginx/sites-available/rshare /etc/nginx/sites-enabled/
sudo nginx -t  # Test the configuration
sudo systemctl restart nginx
```

## Client Setup

The client component runs on your local machine to expose services.

### Installation

1. Install Rust using rustup (if not already installed)
2. Clone and build the rshare repository:

```bash
git clone https://github.com/yourusername/rshare.git
cd rshare
cargo build --release
```

### Using the Client

```bash
# Basic usage with auto-assigned subdomain
./target/release/rshare --port 3000

# Custom subdomain usage
./target/release/rshare --port 3000 --domain your-app.dev.peril.lol

# Specify the server's port (if custom)
./target/release/rshare --port 3000 --domain your-app.dev.peril.lol --public-port 9000
```

Once running, the TUI will display:
- The status of your tunnel
- The public URL to access your service
- Real-time logs of connections

### Keyboard Controls

- `s`: Start/stop the tunnel
- `q`: Quit the application
- `↑/↓`: Scroll through logs

## Troubleshooting

### Common Issues

1. **Connection refused when starting the client:**
   - Ensure the server is running and accessible
   - Check firewall rules on both the server and client

2. **Tunnel starts but connections timeout:**
   - Verify your local service is running on the specified port
   - Check that the port isn't being blocked by a local firewall

3. **DNS not resolving:**
   - Wait for DNS propagation (can take up to 24-48 hours)
   - Verify your DNS records are correctly configured

4. **SSL certificate errors:**
   - Ensure certificates are correctly installed
   - Check that certificate paths in Nginx config are correct
   - Verify certificate hasn't expired

### Logs

For more detailed logs, you can redirect the output to a file:

```bash
# Server
./target/release/rshare --server > server.log 2>&1

# Client
./target/release/rshare --port 3000 > client.log 2>&1
```

## Security Considerations

- The tunnel exposes your local service to the internet - only expose services you intend to share
- Consider implementing authentication for sensitive services
- Keep your rshare installation updated to receive security patches
- Use firewall rules to limit access to your server where appropriate

## Advanced Configuration

For production deployments, consider:

1. Setting up rshare as a systemd service for automatic startup
2. Implementing rate limiting in Nginx to prevent abuse
3. Adding authentication for the tunnel server
4. Monitoring the service with tools like Prometheus and Grafana