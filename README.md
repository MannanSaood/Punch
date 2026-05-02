<div align="center">

# 👊 Punch

**Punches through networks to connect two devices directly.**

No VPN. No account. No cloud middleman. No persistent network overlay.
Just a hole, just for that session.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/core-Rust-orange)](https://www.rust-lang.org/)
[![Server in Go](https://img.shields.io/badge/server-Go-cyan)](https://golang.org/)
[![Version](https://img.shields.io/badge/version-0.4.0-green)]()
[![Open Source](https://img.shields.io/badge/open%20source-yes-brightgreen)]()

[Install](#install) · [Quick Start](#quick-start) · [Commands](#commands) · [Usage Guide](docs/USAGE.md) · [Self-Host](#self-hosting) · [Roadmap](docs/ROADMAP.md)

</div>

---

## What is Punch?

Punch is a lightweight, ephemeral peer-to-peer connectivity tool.

You want to connect two devices on different networks. The usual options are painful:

- **ngrok** — your traffic routes through their servers
- **Tailscale** — create account, install, login, join a persistent VPN mesh
- **Port forwarding** — requires router access, exposes your IP, breaks on dynamic IPs

Punch does one thing: punches a direct hole between two devices and gets out of the way.

```
Device A                  Punch Server               Device B
   |                           |                         |
   |── generate code ─────────▶|                         |
   |◀─ T-No: 4829 ─────────────|                         |
   |                           |◀── connect 4829 ────────|
   |◀──────────── handshake ──────────────────────────▶ |
   |                           |                         |
   |◀══════════ direct p2p connection ════════════════▶ |
   |                    (server exits)                   |
```

The server is a matchmaker. Once the handshake is done, it forgets you both exist.

---

## What can Punch do?

| Feature | Command | Status |
|---------|---------|--------|
| Direct p2p connection | `punch generate` / `punch connect` | ✅ v0.1 |
| Encrypted relay fallback | automatic | ✅ v0.2 |
| Token access control | `--uses N` / `--permanent` | ✅ v0.3 |
| File transfer | `punch send` / `punch receive` | ✅ v0.4 |
| Port forwarding | `punch forward` | 🔜 v0.5 |
| Remote terminal | `punch shell` | 🔜 v0.6 |
| Local dashboard | `punch dashboard` | 🔜 v0.7 |

---

## Install

### Download binary (no setup needed)

Go to [Releases](https://github.com/MannanSaood/Punch/releases) and download for your platform:

| Platform | File |
|----------|------|
| Windows | `punch-windows-x86_64.exe` |
| Linux | `punch-linux-x86_64` |
| macOS (Intel) | `punch-macos-x86_64` |
| macOS (Apple Silicon) | `punch-macos-arm64` |

### Build from source
```bash
git clone https://github.com/MannanSaood/Punch.git
cd Punch/core
cargo build --release
# Binary at: target/release/punch
```

> Note: Punch works best on WiFi. Mobile/corporate networks fall back to encrypted relay.

---

## Quick Start

### Connect two devices
```bash
# Device A — generate a code
punch generate
# → T-No: 4829
# → Waiting for peer...

# Device B — connect
punch connect 4829
# → Punching...
# → ✅ Punched! Direct connection established.
```

### Send a file
```bash
# Device A — send
punch send video.mp4
# → T-No: 6241
# → Waiting for receiver...

# Device B — receive to Downloads folder
punch receive 6241 --dest ~/Downloads
# → Shows consent prompt with file info and risk level
# → Accept? (yes/no): yes
# → Receiving...
```

### Access your home server repeatedly
```bash
# Home server — first time only
punch generate --uses 50
# → Q-No: 7731
punch verify 7731

# Home server — every reconnect
punch listen 7731

# Your laptop — anywhere in the world
punch connect 7731
```

---

## Commands

| Command | Description |
|---------|-------------|
| `punch generate` | Generate a T-No code and wait for peer |
| `punch generate --uses N` | Generate a Q-No code (expires after N uses) |
| `punch generate --permanent` | Generate a P-No permanent token |
| `punch listen <code>` | Reconnect on existing token without consuming a use |
| `punch connect <code>` | Connect to a waiting peer |
| `punch send <file>` | Send a file directly to a peer |
| `punch receive <code>` | Receive a file (saves to current directory) |
| `punch receive <code> --dest <path>` | Receive a file to a specific location |
| `punch verify <code>` | Verify a P-No token before first use |
| `punch revoke <code>` | Revoke a token immediately |
| `punch tokens` | List all active tokens |
| `punch dashboard` | Open local session dashboard |

→ Full reference: [docs/USAGE.md](docs/USAGE.md)

---

## Access Modes

The token type **is** the security policy. No settings menu.

| Mode | Command | Behaviour |
|------|---------|-----------|
| **T-No** | `punch generate` | Temporary. Single session. Expires immediately. |
| **Q-No** | `punch generate --uses 5` | Quantised. Expires after N connections. Persisted locally. |
| **P-No** | `punch generate --permanent` | Permanent. Requires `punch verify` before first use. |

---

## File Transfer

Punch sends files **directly peer to peer** — the server never sees your data.

- **IDM-style parallel streams** — 4 concurrent TCP connections
- **Dynamic chunk sizing** — 1MB to 64MB chunks based on file size
- **Resumable** — drop mid-transfer, reconnect and continue exactly where you left off
- **SHA256 verified** — per chunk and whole file
- **Consent prompt** — receiver sees file name, size, risk level, and fingerprint before accepting
- **Risk classification** — 🔴 executables, 🟡 archives, 🟢 media/documents
- **Acceptance always logged** — `~/.punch/logs/transfers.json`

```bash
# Send
punch send movie.mkv

# Receive to current directory
punch receive 1234

# Receive to specific path
punch receive 1234 --dest "C:\Users\DELL\Downloads"
punch receive 1234 --dest ~/Downloads
```

---

## Connection States

Punch is always transparent about what it's doing:

```
Punching...                          → attempting direct hole punch
✅ Punched! Direct connection.        → p2p established, server is gone
❌ Couldn't punch. Relaying...        → falling back to encrypted relay
🔒 Connected via encrypted relay.     → end-to-end encrypted, server sees nothing
🔑 Keys exchanged. End-to-end encrypted. → X25519 + ChaCha20-Poly1305
```

---

## Philosophy

| Principle | What it means |
|-----------|--------------|
| **Zero knowledge server** | Server facilitates handshake only, never sees your traffic |
| **Zero data stored centrally** | No accounts, no telemetry, no logs on our end |
| **Zero profit** | MIT licensed, no premium tier, no VC money |
| **Ephemeral by default** | Connections exist for a session, not forever |
| **Transparent** | You always know what Punch is doing and why |
| **Local first** | All logs, tokens, and state live on your device only |

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  punch CLI (Rust)                    │
│  connection · file transfer · token enforcement      │
│  X25519 key exchange · ChaCha20 encryption           │
└──────────────────┬──────────────────────────────────┘
                   │ WebSocket (handshake only)
┌──────────────────▼──────────────────────────────────┐
│              signalling server (Go)                  │
│         stateless · matchmaker only · forgets        │
└──────────────────┬──────────────────────────────────┘
                   │ WebSocket (handshake only)
┌──────────────────▼──────────────────────────────────┐
│                  punch CLI (Rust)                    │
│  connection · file transfer · token enforcement      │
└──────────────────┬──────────────────────────────────┘
                   │ local reads only
┌──────────────────▼──────────────────────────────────┐
│               local storage                          │
│  ~/.punch/tokens.json · logs/ · transfers.json       │
└─────────────────────────────────────────────────────┘

File transfer: direct TCP peer to peer (server not involved)
```

---

## Self-Hosting

Run your own signalling server. Punch is fully self-hostable.

```bash
# Docker
docker run -p 8080:8080 mannansaood/punch-server

# Or from source
cd server && go run cmd/main.go

# Point CLI at your server
punch generate --server ws://your-server.com:8080
punch connect 1234 --server ws://your-server.com:8080

# TLS (production)
punch generate --server wss://your-server.com
```

**Environment variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `PUNCH_PORT` | `8080` | Listening port |
| `PUNCH_RELAY_ENABLED` | `true` | Enable encrypted relay fallback |
| `PUNCH_MAX_SESSIONS` | `1000` | Max concurrent sessions |

---

## Local Data

Everything Punch stores lives on your device. Nothing is sent anywhere.

| Path | Contents | Always written? |
|------|----------|-----------------|
| `~/.punch/tokens.json` | Q-No and P-No token state | When tokens created |
| `~/.punch/logs/sessions.json` | Session history | Only with `--log` |
| `~/.punch/logs/transfers.json` | Transfer acceptance record | Always |

---

## Project Structure

```
punch/
├── core/                # Rust — CLI + hole punching + file transfer
│   └── src/
│       ├── main.rs      # Entry point, CLI commands
│       ├── cli.rs       # Command handlers
│       ├── punch.rs     # Hole punching engine
│       ├── stun.rs      # STUN NAT traversal
│       ├── signaling.rs # WebSocket signalling client
│       ├── crypto.rs    # X25519 + ChaCha20 encryption
│       ├── transfer.rs  # IDM-style file transfer
│       ├── safety.rs    # Risk classification + consent
│       ├── token.rs     # Token generation
│       ├── token_store.rs # Token persistence + enforcement
│       └── logger.rs    # Local session logging
├── server/              # Go — signalling server
│   ├── cmd/main.go
│   └── internal/
│       ├── signaling/hub.go
│       ├── relay/
│       └── config/
├── docs/
│   ├── USAGE.md         # Complete command reference
│   ├── SRS.md           # Software requirements spec
│   ├── ROADMAP.md       # Version roadmap
│   └── CONTRIBUTING.md  # Contribution guide
├── .github/workflows/
│   ├── ci.yml           # Build on every push
│   └── release.yml      # Release binaries on tag
├── render.yaml          # Render deployment config
├── Dockerfile           # Server container
└── README.md
```

---

## Roadmap

```
v0.1 ✅  Connection + relay fallback
v0.2 ✅  Encrypt relay (X25519 + ChaCha20-Poly1305)
v0.3 ✅  Token enforcement (T-No, Q-No, P-No) + listen command
v0.4 ✅  File transfer — IDM chunked, resumable, safe
v0.5 🔜  Port forwarding — punch forward <port> <code>
v0.6 🔜  Remote terminal — punch shell + consent + monitoring
v0.7 🔜  Local dashboard — session and token visualisation
v0.8 🔜  Developer library — punch-core on crates.io
v1.0 🔜  Public launch
```

---

## Contributing

Read [CONTRIBUTING.md](docs/CONTRIBUTING.md) before opening a PR.

Actively needed:
- Symmetric NAT edge case testing
- Windows testing and bug reports
- v0.5 port forwarding implementation
- Dashboard UI (Svelte)

---

## License

MIT — do whatever you want, just don't sell it as a closed source product.

---

<div align="center">

Built by [Syed Mannan Saood](https://github.com/MannanSaood) · Bengaluru, India

*Zero knowledge. Zero profit. Zero compromise.*

</div>