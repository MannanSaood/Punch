# Punch — Roadmap
> Ship something real at every milestone. No milestone ends in "in progress."

---

## v0.1 — Core Connection
Two devices. One code. Direct connection.

- WebSocket signalling server (Go) — stateless, matchmaker only
- UDP hole punching via STUN
- Encrypted relay fallback (automatic)
- T-No ephemeral tokens
- `--server` flag for self-hosted instances
- Single binary + Dockerfile

---

## v0.2 — Encrypted Relay
Zero knowledge claim is now actually true.

- X25519 Diffie-Hellman key exchange via signalling server
- ChaCha20-Poly1305 end-to-end encryption on relay traffic
- Server forwards encrypted gibberish it cannot read
- Both peers independently derive same secret — server never has it

---

## v0.3 — Token Enforcement
Access control that lives on your device.

- T-No — ephemeral, nothing stored
- Q-No — usage counter in `~/.punch/tokens.json`, persists across restarts
- P-No — stored, blocked until `punch verify`
- `punch listen` — reconnect without burning a token use
- `punch tokens` / `punch revoke` / `punch verify`
- Enforcement on generator side (Device A owns the policy)

---

## v0.4 — File Transfer
Direct peer to peer. Resumable. Safe.

- Iroh QUIC transport — hole punch + relay.iroh.network fallback
- IDM-style parallel chunked transfer (4 streams)
- Dynamic chunk sizing (1MB / 4MB / 16MB / 64MB by file size)
- SHA256 per chunk + whole file
- Resumable via `.punch_partial` + `.punch_state`
- Idempotent chunks — ACK-lost scenario handled correctly
- Connection drop vs data corruption distinguished
- Risk classification — high / medium / low
- Consent prompt with 30s timeout
- Session fingerprint for verbal verification
- Acceptance always logged

---

## v0.5 — Port Forwarding
Any port. TCP + UDP. No bottleneck.

- Iroh QUIC — same stack as file transfer, same connectivity guarantees
- TCP: each local connection = one Iroh bidirectional stream
- UDP: unreliable datagrams (preserves UDP semantics exactly)
- Handles both `127.0.0.1` and `[::1]` — works with Vite, webpack, etc.
- In-band handshake verification — port whitelist enforced at protocol level
- Session fingerprint — verbal MITM check
- Max 50 concurrent streams — DoS protection
- T-No / Q-No / P-No token support
- Full audit log at `~/.punch/logs/forward.json`
- 256KB I/O buffer — tuned for streaming throughput

---

## v0.6 — Remote Terminal
Secure shell access over **Iroh QUIC**. Explicit host consent. Local visibility and audit on the host.

```bash
# Device B — host (machine that runs the shell)
punch shell host
punch shell host --uses 10 --permanent   # Q-No / P-No options (same pattern as forward)

# Device A — client
punch shell connect <code>
```

**Shipped behaviour:**
- WebSocket signalling message type `shell` (handshake with `EndpointAddr` + fingerprint)
- **Iroh QUIC** data plane — same connectivity as file transfer / port forward (direct or relay)
- **Host:** consent prompts before the PTY starts; live command monitor; **Ctrl+K** kills session
- **Client:** fingerprint shown for verbal check with host; interactive terminal after approval
- **portable-pty** + **crossterm** for PTY and local terminal modes; configurable blocklist / suspicious patterns via `shell_config`
- Host session log: **`~/.punch/logs/shell_sessions.json`**

**Future polish (post-v0.6):** `punch shell list` (active sessions), richer “persist on disconnect” behaviour, dashboard integration (v0.7).

---

## v0.7 — Local Dashboard
See everything Punch has ever done. On your machine only.

- Svelte static bundle served locally
- Connection history, token activity, and transfer logs
- Zero external requests

---

## v0.8 — Data Piping (Shipped)
Stream raw data directly between terminals.

- `punch pipe send` / `punch pipe receive`
- Iroh QUIC transport for stdin/stdout streams
- Ideal for logs, database dumps, and CI/CD pipelines

---

## v0.9 — Developer Library (planned)
Embed Punch in your own apps.

- Extract core into `punch-core` crate
- Clean async API: `connect()`, `send()`, `recv()`, `forward()`, `close()`
- Published to crates.io

---

## v0.10 — Sidecar (planned)
Headless background service for complex integrations.

- REST + WebSocket API over local port
- Named sessions and persistent background connections
- Programmatic control over Punch connections

---

## v1.0 — Public Launch (planned)
Production ready. Real world tested. Properly distributed.

---

**Hardening:**
- Security audit of relay encryption
- Fuzz testing on STUN and protocol parsing
- Load test: 1000 concurrent sessions
- Symmetric NAT comprehensive testing
- Windows full test suite
- ARM builds (Raspberry Pi)

**Distribution:**
- `cargo install punch-cli`
- Homebrew formula
- Prebuilt binaries on GitHub Releases (all platforms)
- Docker Hub for server

**Documentation:**
- Full docs site (GitHub Pages)
- Protocol specification (RFC-style)
- Security model writeup
- Self-hosting guide

**Done when:** Someone who has never heard of Punch installs it, connects two devices, and understands what happened — in under 5 minutes.

---

## Post v1.0 — Future Ideas
Not committed. Only after v1.0 ships.

- **QUIC upgrade for file transfer** — already on Iroh, potential direct Quinn for more control
- **Multi-peer sessions** — connect more than two devices
- **`punch sync`** — folder sync between devices
- **Mobile CLI** — Termux support for Android
- **Prometheus metrics** — for self-hosters
- **`punch broadcast`** — one sender, multiple receivers

---

## Summary

| Version | Status | Key Deliverable |
|---------|--------|-----------------|
| v0.1 | Shipped | P2P connection works |
| v0.2 | Shipped | Relay encrypted, zero knowledge true |
| v0.3 | Shipped | Token enforcement, listen command |
| v0.4 | Shipped | File transfer, Iroh QUIC, resumable |
| v0.5 | Shipped | Port forwarding, TCP + UDP |
| v0.6 | Shipped | Remote terminal + consent |
| v0.7 | Shipped | Local dashboard |
| v0.8 | Shipped | Data piping (stdin/stdout) |
| v0.9 | Planned | Developer library (`punch-core`) |
| v0.10 | Planned | Sidecar API (REST/WS) |
| v1.0 | Planned | Public launch |