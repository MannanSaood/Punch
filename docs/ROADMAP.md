# Punch — Roadmap

> Philosophy: Ship something real at every milestone. No milestone ends in "in progress."

---

## v0.1 — Proof of Concept
**Goal: Two devices. One command. Direct connection.**  
*The only thing that matters here is proving the core works.*

### Server (Go)
- [ ] WebSocket server scaffolding
- [ ] In-memory session matchmaking (token → two peers)
- [ ] STUN endpoint exchange between matched peers
- [ ] Session timeout and cleanup (30s handshake window)
- [ ] `/health` endpoint
- [ ] Single binary build + Dockerfile

### Core CLI (Rust)
- [ ] `punch generate` — generate T-No code, display it, wait
- [ ] `punch connect <code>` — connect to waiting peer
- [ ] STUN discovery (public IP + port)
- [ ] UDP hole punching attempt
- [ ] Connection state messages in terminal
- [ ] Startup network advisory message
- [ ] `--server` flag for custom signalling server

### Done when:
> Two laptops on different home WiFi networks connect directly in under 3 seconds using `punch generate` and `punch connect`.

---

## v0.2 — Relay Fallback
**Goal: Punch works even when it can't punch.**

### Server (Go)
- [ ] Encrypted relay pathway
- [ ] Relay traffic forwarding (opaque bytes only)
- [ ] Relay session lifecycle management

### Core CLI (Rust)
- [ ] Detect hole punch failure (5s timeout)
- [ ] Automatic relay fallback
- [ ] "Couldn't punch. Relaying encrypted traffic." message
- [ ] ChaCha20-Poly1305 end-to-end encryption for relay traffic

### Done when:
> Two devices on symmetric NAT (corporate/mobile) connect via relay. Server operator cannot read payload.

---

## v0.3 — Access Modes
**Goal: T-No, Q-No, P-No all working.**

### Core CLI (Rust)
- [ ] `punch generate --uses N` → Q-No token
- [ ] `punch generate --permanent` → P-No token
- [ ] Q-No usage counter enforcement (client-side)
- [ ] P-No verification prompt on generating device
- [ ] Token expiry handling and clear error messages

### Done when:
> All three token types work end to end with correct expiry behaviour.

---

## v0.4 — Local Logging + Dashboard
**Goal: You can see what Punch did, on your machine only.**

### Core CLI (Rust)
- [ ] Opt-in local session logging (`--log` flag)
- [ ] Log to `~/.punch/logs/sessions.json`
- [ ] Log schema: session_id, token type, connection type, timestamps, bytes
- [ ] `punch dashboard` command — starts local Svelte server

### Dashboard (Svelte)
- [ ] Session history list
- [ ] Connection type breakdown (direct vs relay)
- [ ] Token activity view (Q-No remaining uses, P-No last used)
- [ ] Data transferred per session
- [ ] Zero external network requests
- [ ] Compiled static build, no CDN

### Done when:
> `punch dashboard` opens a browser showing full session history. Wireshark confirms zero external requests from dashboard.

---

## v0.5 — Developer Library
**Goal: Other developers can embed Punch in their apps.**

### Library (Rust crate)
- [ ] Extract core into `punch-core` crate
- [ ] Clean public API: `connect()`, `send()`, `recv()`, `close()`
- [ ] Async/Tokio throughout
- [ ] Published to crates.io
- [ ] Docs with examples on docs.rs
- [ ] Integration test suite

### Done when:
> Someone outside the project builds a working p2p app using `punch-core` from crates.io with < 20 lines of code.

---

## v1.0 — Public Launch
**Goal: Production ready. Real world tested. Documented.**

### Hardening
- [ ] Security audit of relay encryption
- [ ] Fuzz testing on STUN parsing
- [ ] Load test signalling server (1000 concurrent sessions)
- [ ] Windows CLI support
- [ ] ARM builds (Raspberry Pi homelab users)

### Distribution
- [ ] `cargo install punch-cli`
- [ ] Homebrew formula
- [ ] Prebuilt binaries via GitHub releases (Linux x86_64, ARM64, macOS, Windows)
- [ ] Docker Hub image for server

### Documentation
- [ ] Full docs site (GitHub Pages)
- [ ] Self-hosting guide
- [ ] Protocol specification (RFC-style)
- [ ] Security model writeup

### Done when:
> Punch is publicly announced. Someone who has never heard of it can install it, connect two devices, and understand what happened — in under 5 minutes.

---

## Post v1.0 — Ideas (Not Committed)

These are things worth thinking about but not building until v1.0 ships:

- **Multi-peer sessions** — connect more than two devices
- **File transfer mode** — `punch send file.zip <code>`
- **Port forwarding mode** — expose a specific local port only
- **eBPF instrumentation** — kernel-level connection diagnostics in dashboard
- **Prometheus metrics** — for self-hosters who want server observability
- **Mobile CLI** — Termux support for Android

---

## Milestone Summary

| Version | Focus | Key Deliverable |
|---------|-------|-----------------|
| v0.1 | Core | Direct p2p connection works |
| v0.2 | Reliability | Relay fallback works, stays zero knowledge |
| v0.3 | Access Control | All three token modes work |
| v0.4 | Observability | Local dashboard live |
| v0.5 | Ecosystem | Developer library on crates.io |
| v1.0 | Launch | Production ready, publicly announced |
