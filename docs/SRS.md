# Punch — Software Requirements Specification (SRS)
**Version:** 1.0  
**Author:** Syed Mannan Saood  
**Status:** Draft  
**Last Updated:** April 2026

---

## 1. Introduction

### 1.1 Purpose
This document defines the complete software requirements for **Punch** — a zero-knowledge, ephemeral peer-to-peer connectivity tool that enables direct connections between devices on different networks without VPNs, accounts, or persistent network overlays.

### 1.2 Scope
Punch consists of four components:
- A **signalling server** (Go) — stateless matchmaker
- A **core CLI** (Rust) — hole punching, relay fallback, token management
- A **local dashboard** (Svelte) — session observability, local logs only
- A **developer library** (Rust) — embeddable core for third-party apps

### 1.3 Definitions

| Term | Definition |
|------|-----------|
| **Hole Punching** | NAT traversal technique where two peers simultaneously send packets to each other's public endpoints to establish a direct path |
| **STUN** | Session Traversal Utilities for NAT — protocol to discover public IP/port |
| **Symmetric NAT** | A NAT type that assigns a different external port for each destination, blocking hole punching |
| **Relay** | A server that forwards encrypted traffic when direct connection fails |
| **T-No** | Temporary access token — single session, expires immediately after |
| **Q-No** | Quantised access token — expires after N connections |
| **P-No** | Permanent access token — persistent, requires additional verification |
| **Signalling** | The process of exchanging network metadata (IPs, ports) to facilitate peer connection |
| **Zero Knowledge** | The server never sees payload traffic, only facilitates the handshake |

### 1.4 References
- RFC 5389 — STUN Protocol
- RFC 5766 — TURN Protocol  
- RFC 8445 — ICE Protocol
- WebRTC NAT Traversal Specification

---

## 2. Overall Description

### 2.1 Product Perspective

Punch sits in the gap between:
- **Too heavy**: Tailscale, WireGuard (full VPN mesh, persistent, identity-tied)
- **Too coupled**: ngrok (cloud-dependent, traffic through their servers)
- **Too manual**: Port forwarding (router access, static IPs, maintenance)

Punch is ephemeral, zero-trust, and self-hostable. It does one thing exceptionally well.

### 2.2 Product Philosophy

1. **Ephemeral by default** — connections exist for a session, not forever
2. **Zero knowledge** — the server is a matchmaker, not a participant
3. **Zero data** — no accounts, no telemetry, no central logs
4. **Zero profit** — MIT licensed, fully open source, no premium tier
5. **Transparent** — users always know the connection state and why

### 2.3 User Classes

| User | Description |
|------|-------------|
| **Developer** | Wants to share localhost, embed Punch in their app, use CLI in scripts |
| **Homelab user** | Wants devices to connect without VPN or port forwarding |
| **Power user** | Self-hosts the server, audits logs via dashboard |
| **Open source contributor** | Builds on or contributes to Punch |

### 2.4 Operating Environment
- **CLI**: Linux, macOS, Windows (primary: Linux/macOS)
- **Server**: Linux (Docker supported)
- **Dashboard**: Any modern browser (runs locally)
- **Network**: WiFi preferred; Mobile/corporate may fall back to relay

---

## 3. Functional Requirements

### 3.1 Signalling Server (Go)

#### FR-S1: Session Matchmaking
The server SHALL accept WebSocket connections from peers and match them using a shared token code.

#### FR-S2: Stateless Operation
The server SHALL NOT persist any session data to disk or database. All state is in-memory and discarded after handshake completion.

#### FR-S3: Token Validation
The server SHALL validate token format but SHALL NOT store token history or usage counts. Token policy enforcement happens on the client.

#### FR-S4: Handshake Facilitation
The server SHALL exchange STUN-derived public endpoint information between matched peers and immediately terminate its involvement post-handshake.

#### FR-S5: Encrypted Relay Fallback
The server SHALL provide a relay pathway for peers that cannot establish direct connections. The server SHALL only forward encrypted bytes and SHALL have no ability to decrypt payload traffic.

#### FR-S6: Concurrency
The server SHALL handle multiple simultaneous sessions using Go goroutines. No external queue is required for v1.

#### FR-S7: Self-Hostable
The server SHALL be deployable via a single binary or Docker container with zero external dependencies.

#### FR-S8: Configuration
The server SHALL be configurable via environment variables:
- `PUNCH_PORT` — listening port (default: 8080)
- `PUNCH_RELAY_ENABLED` — enable/disable relay fallback (default: true)
- `PUNCH_MAX_SESSIONS` — max concurrent sessions (default: 1000)

---

### 3.2 Core CLI (Rust)

#### FR-C1: Token Generation
The CLI SHALL generate a numeric T-No code (4-6 digits) upon `punch generate`.

#### FR-C2: Token Types
The CLI SHALL support three token modes:
- `punch generate` → T-No (single session)
- `punch generate --uses N` → Q-No (N sessions)
- `punch generate --permanent` → P-No (persistent, with verification)

#### FR-C3: Connection Initiation
The CLI SHALL connect to a peer using `punch connect <code>` and attempt hole punching first.

#### FR-C4: NAT Traversal
The CLI SHALL use STUN to discover its public IP and port, then attempt UDP hole punching with the peer.

#### FR-C5: Relay Fallback
If hole punching fails after a configurable timeout (default: 5s), the CLI SHALL automatically fall back to the encrypted relay without user intervention.

#### FR-C6: Connection State Messaging
The CLI SHALL display clear human-readable state messages at every stage:
```
Punching...
Punched! Direct connection.
Couldn't punch. Relaying encrypted traffic.
Connected via encrypted relay.
```

#### FR-C7: Startup Warning
The CLI SHALL display a network advisory at startup:
```
Note: Punch works best on WiFi.
Mobile/corporate networks may fall back to relay.
```

#### FR-C8: Custom Server
The CLI SHALL accept a `--server` flag to point at a self-hosted signalling server:
```bash
punch connect 4829 --server wss://your-server.com
```

#### FR-C9: Local Session Logging
The CLI SHALL optionally write session logs to a local file (`~/.punch/logs/sessions.json`). Logging SHALL be opt-in, disabled by default.

#### FR-C10: Log Schema
Each log entry SHALL contain:
```json
{
  "session_id": "uuid",
  "token_type": "T-No | Q-No | P-No",
  "token_code": "4829",
  "connection_type": "direct | relay",
  "started_at": "ISO8601",
  "ended_at": "ISO8601",
  "duration_seconds": 142,
  "bytes_sent": 1024000,
  "bytes_received": 2048000,
  "peer_region": "inferred from STUN"
}
```

#### FR-C11: P-No Verification
Permanent tokens SHALL require a secondary verification step — a confirmation prompt on the generating device before the connection is accepted.

---

### 3.3 Local Dashboard (Svelte)

#### FR-D1: Local Only
The dashboard SHALL run entirely on localhost (`http://localhost:7777`). It SHALL NOT make any external network requests.

#### FR-D2: Log Reading
The dashboard SHALL read session logs from `~/.punch/logs/sessions.json` only.

#### FR-D3: Session History
The dashboard SHALL display:
- Session list with timestamps
- Connection type (direct / relay)
- Token type used
- Duration and data transferred
- Per-session breakdown

#### FR-D4: Token Activity
The dashboard SHALL show active Q-No and P-No tokens, their remaining uses, and last used timestamp.

#### FR-D5: No External Dependencies at Runtime
The dashboard SHALL be a compiled static build. No CDN calls, no external fonts, no analytics.

---

### 3.4 Developer Library (Rust)

#### FR-L1: Embeddable Core
The library SHALL expose the core hole punching and relay logic as a Rust crate publishable to crates.io.

#### FR-L2: Simple API
```rust
let conn = punch::connect("4829").await?;
conn.send(data).await?;
let received = conn.recv().await?;
```

#### FR-L3: Async First
The library SHALL be built on Tokio and expose async interfaces throughout.

---

## 4. Non-Functional Requirements

### 4.1 Performance
- Handshake to direct connection: **< 3 seconds** on WiFi
- Handshake to relay connection: **< 8 seconds**
- Server memory per session: **< 1MB**
- CLI binary size: **< 10MB**

### 4.2 Security
- All relay traffic SHALL be end-to-end encrypted (ChaCha20-Poly1305)
- T-No codes SHALL be randomly generated, not sequential
- P-No tokens SHALL require explicit device-side confirmation
- No credentials, emails, or personal data SHALL ever be collected

### 4.3 Reliability
- Server SHALL handle session timeout gracefully (default: 30s handshake timeout)
- CLI SHALL retry STUN discovery up to 3 times before falling back to relay
- Relay fallback SHALL be automatic and transparent to the user

### 4.4 Portability
- Server deployable via single binary or Docker
- CLI distributed via `cargo install`, Homebrew, and prebuilt binaries
- No runtime dependencies required on client machines

### 4.5 Observability
- Server SHALL expose a `/health` endpoint
- Server SHALL expose a `/metrics` endpoint (Prometheus-compatible) as opt-in
- CLI SHALL support `--verbose` flag for debug output

---

## 5. System Constraints

### 5.1 What Punch Will NOT Do
- Store any user data centrally
- Require account creation or email verification
- Maintain persistent network overlays
- Charge money or have a premium tier
- Operate as a general-purpose VPN

### 5.2 Known Limitations
- Symmetric NAT will prevent direct connection (~15-20% of cases, mostly mobile/corporate)
- Relay fallback introduces latency overhead
- P-No tokens require both devices to have Punch running

---

## 6. Data Flow

### 6.1 T-No Connection Flow
```
1. Device A: punch generate
2. Server: assigns code 4829, holds session slot
3. Device A: displays "T-No: 4829, Waiting..."
4. Device B: punch connect 4829
5. Server: matches both peers, exchanges STUN endpoints
6. Both: attempt simultaneous UDP hole punch
7a. Success: direct p2p established, server session destroyed
7b. Failure: relay pathway opened, encrypted traffic forwarded
8. Session ends: server slot freed, local logs written if enabled
```

### 6.2 Data the Server Sees
```
Sees: Connection timestamp
Sees: Token code (temporary, in-memory only)
Sees: STUN-derived public IPs (temporary, for handshake only)
Does not see: Payload traffic (never)
Does not see: User identity (never)
Does not see: Persistent logs (never)
```

---

## 7. Acceptance Criteria

| ID | Criteria |
|----|---------|
| AC-1 | Two devices on different WiFi networks connect in < 3 seconds |
| AC-2 | Two devices on symmetric NAT connect via relay in < 8 seconds |
| AC-3 | Server binary runs with zero config on a fresh Linux machine |
| AC-4 | Dashboard loads and shows session history with no internet connection |
| AC-5 | P-No token requires explicit confirmation before accepting connection |
| AC-6 | `punch connect` with custom `--server` flag works against self-hosted instance |
| AC-7 | Relay traffic cannot be decrypted by the server operator |
