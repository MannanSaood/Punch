# Deploy Punch server on Oracle VM (with WSS)

Clients use the signalling URL **without** a path: `wss://your-domain.example` (the CLI appends `/ws`).

## 1. Domain and DNS

1. Register or use a hostname, e.g. `punch.example.com`.
2. Create an **A record** pointing to your Oracle VM **public IP**.
3. Wait until DNS resolves: `dig +short punch.example.com` shows the VM IP.

Let's Encrypt needs a real domain; IP-only HTTPS is not supported by this setup.

## 2. Oracle Cloud networking

In the VCN **security list** (or NSG on the instance):

| Direction | Port | Purpose |
|-----------|------|---------|
| Ingress | 22 | SSH |
| Ingress | 80 | HTTP (ACME + redirect to HTTPS) |
| Ingress | 443 | HTTPS / WSS |

You do **not** need to expose **8080** publicly; Caddy talks to the container on the Docker network.

On the VM (if `ufw` is enabled):

```bash
sudo ufw allow 22/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

## 3. VM one-time setup

```bash
sudo apt update && sudo apt install -y docker.io docker-compose-v2 curl
sudo usermod -aG docker $USER
# log out and back in

sudo mkdir -p /opt/punch
sudo chown "$USER:$USER" /opt/punch
```

Add your SSH **public** key to `~/.ssh/authorized_keys` (GitHub Actions uses the matching **private** key in `ORACLE_SSH_KEY`).

## 4. GitHub Actions secrets

| Secret | Example |
|--------|---------|
| `ORACLE_HOST` | VM public IP (for SSH) |
| `ORACLE_USER` | `ubuntu` |
| `ORACLE_SSH_KEY` | Private key PEM |
| `PUNCH_DOMAIN` | `punch.example.com` (no `https://`, no trailing slash) |

Push to `main` (with `server/` or `deploy/` changes) or run **Build and Deploy Punch Server** manually.

## 5. Verify WSS

```bash
curl -fsS "https://punch.example.com/health"
# {"status":"ok","service":"punch-signalling"}

# Optional: websocket check (needs websocat or similar)
# websocat -v "wss://punch.example.com/ws"
```

CLI:

```bash
punch generate --server wss://punch.example.com
punch connect 1234 --server wss://punch.example.com
```

## 6. Manual deploy on the VM (without Actions)

```bash
cd /opt/punch
# copy deploy/Caddyfile and deploy/docker-compose.yml here
cat > .env <<EOF
PUNCH_IMAGE=ghcr.io/YOUR_USER/punch:latest
PUNCH_DOMAIN=punch.example.com
EOF
docker login ghcr.io
docker compose pull
docker compose up -d
```

## Troubleshooting

- **Certificate errors:** DNS must point at this VM before the first deploy; retry `docker compose up -d` after DNS propagates.
- **502 on /ws:** `docker logs punch-server` and `docker logs punch-caddy`.
- **Firewall:** confirm 443 is open in Oracle console, not only on the VM.
