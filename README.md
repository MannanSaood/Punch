# 👊 Punch

> **Punches through networks to connect two devices directly.**

No VPN. No account. No cloud middleman. No persistent network overlay.  
Just a hole, just for that session.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/core-Rust-orange)](https://www.rust-lang.org/)
[![Server in Go](https://img.shields.io/badge/server-Go-cyan)](https://golang.org/)
[![Open Source](https://img.shields.io/badge/open%20source-yes-green)]()

---

## The Problem

You want to connect two devices on different networks.

- **ngrok** — your traffic routes through their servers. Not your data.
- **Tailscale** — create account, install, login, get assigned a virtual IP, join a persistent VPN mesh. Just to share a port.
- **Port forwarding** — requires router access, exposes your IP, breaks on dynamic IPs.

All of this is too much. Punch does one thing: it punches a direct hole between two devices and gets out of the way.

---

## How It Works

```
Device A                  Punch Server               Device B
   |                           |                         |
   |── generate code ─────────▶|                         |
   |◀─ T-No: 4829 ─────────────|                         |
   |                           |◀── connect 4829 ────────|
   |◀──────────── handshake ───────────────────────────▶|
   |                           |                         |
   |◀═══════════ direct p2p connection ════════════════▶|
   |                    (server exits)                   |
```

The server is a matchmaker. Once the handshake is done, it forgets you both exist.

---

## Quick Start

```bash
# Install
cargo install punch-cli

# On Device A — generate a code
punch generate

# Output:
# T-No: 4829
# Waiting for peer...

# On Device B — connect using the code  
punch connect 4829

# Output:
# Punching...
# Punched! Direct connection established.
```

---

## Access Modes

Punch has three token types. The token type **is** the security policy.

| Mode | Command | Behaviour |
|------|---------|-----------|
| **T-No** | `punch generate` | Temporary. Expires after session ends. One time use. |
| **Q-No** | `punch generate --uses 5` | Quantised. Expires after N connections. |
| **P-No** | `punch generate --permanent` | Permanent. Requires extra verification step. |

---

## Connection States

Punch is always transparent about what it's doing:

```
Punching...                     → Attempting direct hole punch
Punched! Direct connection.     → Success. P2P. Server is gone.
Couldn't punch. Relaying...     → Falling back to encrypted relay
Connected via encrypted relay.  → End-to-end encrypted. Server sees nothing.
```

> **Note:** Punch works best on WiFi. Mobile (4G/5G) and corporate networks
> may fall back to encrypted relay due to symmetric NAT restrictions.

---

## Self-Hosting the Server

Punch is fully self-hostable. You don't have to trust anyone's infrastructure.

```bash
# Clone and run
git clone https://github.com/MannanSaood/punch
cd punch/server
go run cmd/main.go

# Or with Docker
docker run -p 8080:8080 mannansaood/punch-server

# Point your CLI at your own server
punch connect 4829 --server wss://your-server.com
```

---

## Local Dashboard

Punch optionally keeps session logs **on your device only**. Nothing is sent anywhere.

```bash
punch dashboard
# Opens local dashboard at http://localhost:7777
```

View session history, connection types, data transferred, access token activity. Your logs, your machine, your eyes only.

---

## Philosophy

- **Zero knowledge server** — the signalling server never sees your traffic
- **Zero data stored centrally** — no accounts, no telemetry, no logs on our end
- **Zero profit** — fully open source, no premium tier, no VC money
- **Ephemeral by default** — connections exist for a session, not forever
- **Transparent** — you always know what Punch is doing and why

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                     punch CLI (Rust)                 │
│         hole punching · relay fallback · logging     │
└──────────────────┬──────────────────────────────────┘
                   │ WebSocket
┌──────────────────▼──────────────────────────────────┐
│              signalling server (Go)                  │
│         stateless · matchmaker only · forgets        │
└──────────────────┬──────────────────────────────────┘
                   │ WebSocket
┌──────────────────▼──────────────────────────────────┐
│                     punch CLI (Rust)                 │
│         hole punching · relay fallback · logging     │
└──────────────────┬──────────────────────────────────┘
                   │ reads local logs only
┌──────────────────▼──────────────────────────────────┐
│               dashboard (Svelte)                     │
│          local · offline · your device only          │
└─────────────────────────────────────────────────────┘
```

---

## Project Structure

```
punch/
├── server/              # Go — signalling server
│   ├── cmd/             # Entry point
│   └── internal/
│       ├── signaling/   # WebSocket session matchmaking
│       ├── relay/       # Encrypted relay fallback
│       └── config/      # Server configuration
├── core/                # Rust — CLI + hole punching
│   └── src/
├── dashboard/           # Svelte — local session dashboard
│   └── src/
└── docs/                # SRS, Roadmap, RFC documents
```

---

## Contributing

Punch is open source and welcomes contributions. Read [CONTRIBUTING.md](docs/CONTRIBUTING.md) before opening a PR.

Areas actively needing help:
- STUN implementation hardening
- Symmetric NAT edge cases
- Dashboard UI improvements
- Windows testing

---

## License

MIT — do whatever you want, just don't sell it as a closed source product.

---

*Built by [Syed Mannan Saood](https://mannansaood.vercel.app/) · Bengaluru, India*
